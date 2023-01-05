use std::collections::HashMap;

use num::{BigUint, One, Zero};
use twenty_first::amount::u32s::U32s;
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::rescue_prime_digest::Digest;
use twenty_first::shared_math::rescue_prime_regular::{RescuePrimeRegular, DIGEST_LENGTH};
use twenty_first::util_types::algebraic_hasher::{AlgebraicHasher, Hashable};
use twenty_first::util_types::mmr;
use twenty_first::util_types::mmr::mmr_membership_proof::MmrMembershipProof;

use crate::arithmetic::u32s_2::sub::U32s2Sub;
use crate::library::Library;
use crate::list::u32::pop::Pop;
use crate::list::u32::set_length::{self, SetLength};
use crate::rust_shadowing_helper_functions;
use crate::snippet_trait::Snippet;
use crate::{arithmetic::u32s_2::powers_of_two::U32s2PowersOfTwoStatic, list::u32::push::Push};

use super::right_ancestor_count_and_own_height::{self, MmrRightAncestorCountAndHeight};
use super::{
    data_index_to_node_index::{self, DataIndexToNodeIndex},
    right_child_and_height::MmrRightChildAndHeight,
};

pub const MAX_MMR_HEIGHT: usize = 64;

pub struct CalculateNewPeaksFromAppend;

impl Snippet for CalculateNewPeaksFromAppend {
    fn stack_diff() -> isize {
        // pops: `old_leaf_count` (u32s<2>); old_peaks (*list); [digests (new_leaf)]
        // pushes: *list (new peaks); *auth_path_of_newly_added_leaf
        -6
    }

    fn entrypoint() -> &'static str {
        "calculate_new_peaks_from_append"
    }

    fn function_body(library: &mut Library) -> String {
        let entrypoint = Self::entrypoint();
        let data_index_to_node_index = library.import::<DataIndexToNodeIndex>();
        let right_ancestor_count_and_own_height =
            library.import::<MmrRightAncestorCountAndHeight>();
        let push = library.import::<Push<DIGEST_LENGTH>>();
        let pop = library.import::<Pop<DIGEST_LENGTH>>();
        let set_length = library.import::<SetLength>();

        // Allocate memory for the returned auth path for the newly inserted element
        // Warning: This auth path is only allocated *once* even though the code is called multiple times.
        // So if this function is called multiple times, the auth_paths will be overwritten.
        let static_auth_path_pointer = library.kmalloc(DIGEST_LENGTH * MAX_MMR_HEIGHT);

        format!(
            "
                // BEFORE: _ old_leaf_count_hi old_leaf_count_lo *peaks [digests (new_leaf)]
                // AFTER: _ *new_peaks *auth_path
                {entrypoint}:
                    call {push}
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *peaks

                    // Create auth_path return value (vector living in RAM)
                    // Notice that this will always allocate to the same pointer in memory, even
                    // if this function is called multiple times.
                    push {static_auth_path_pointer}
                    push 0
                    call {set_length}
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *peaks *auth_path

                    swap1
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks

                    dup3 dup3
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks old_leaf_count_hi old_leaf_count_lo

                    call {data_index_to_node_index}
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks new_ni_hi new_ni_lo

                    call {right_ancestor_count_and_own_height}
                    pop
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks rac

                    call {entrypoint}_while
                    // stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks (rac = 0)

                    pop
                    swap3 pop swap1 pop
                    // stack: _ *peaks *auth_path

                    return

                // Stack start and end: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks rac
                {entrypoint}_while:
                    dup0
                    push 0
                    eq
                    skiz
                        return
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks rac

                    swap2 swap1
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks

                    dup0
                    call {pop}
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks [digest (new_hash)]

                    dup5
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks [digest (new_hash)] *peaks

                    call {pop}
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks [digest (new_hash)] [digests (previous_peak)]

                    // Update authentication path with latest previous_peak
                    dup11
                    dup5 dup5 dup5 dup5 dup5
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks [digest (new_hash)] [digests (previous_peak)] *auth_path [digests (previous_peak)]

                    call {push}
                    pop
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks [digest (new_hash)] [digests (previous_peak)]

                    hash
                    pop pop pop pop pop
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks [digests (new_peak)]

                    call {push}
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo rac *auth_path *peaks

                    swap1 swap2
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks rac

                    push -1
                    add
                    // Stack: _ old_leaf_count_hi old_leaf_count_lo *auth_path *peaks (rac - 1)

                    recurse
                "
        )
    }

    fn rust_shadowing(
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        type H = RescuePrimeRegular;

        // TODO: Remove this when twenty-first is updated
        fn right_ancestor_count_and_own_height(node_index: u128) -> (u32, u32) {
            let (mut candidate, mut candidate_height) = mmr::shared::leftmost_ancestor(node_index);

            // leftmost ancestor is always a left node, so count starts at 0.
            let mut right_ancestor_count = 0;

            loop {
                if candidate == node_index {
                    return (right_ancestor_count, candidate_height as u32);
                }

                let left_child = mmr::shared::left_child(candidate, candidate_height);
                let candidate_is_right_child = left_child < node_index;
                if candidate_is_right_child {
                    candidate = mmr::shared::right_child(candidate);
                    right_ancestor_count += 1;
                } else {
                    candidate = left_child;
                    right_ancestor_count = 0;
                };

                candidate_height -= 1;
            }
        }

        // BEFORE: _ old_leaf_count_hi old_leaf_count_lo *peaks [digests (new_leaf)]
        // AFTER: _ *new_peaks *auth_path
        let new_leaf: Digest = Digest::new([
            stack.pop().unwrap(),
            stack.pop().unwrap(),
            stack.pop().unwrap(),
            stack.pop().unwrap(),
            stack.pop().unwrap(),
        ]);
        let peaks_pointer = stack.pop().unwrap();
        let old_leaf_count_lo = stack.pop().unwrap().value();
        let old_leaf_count_hi = stack.pop().unwrap().value();
        let old_leaf_count = (old_leaf_count_hi << 32) | old_leaf_count_lo;

        let mut old_peaks: Vec<Digest> = vec![];
        let peak_count = memory[&peaks_pointer].value() as u32;

        for i in 0..peak_count {
            let offset = BFieldElement::new((i as usize * DIGEST_LENGTH) as u64);
            old_peaks.push(Digest::new([
                memory[&(peaks_pointer + offset + BFieldElement::one())],
                memory[&(peaks_pointer + offset + BFieldElement::new(2))],
                memory[&(peaks_pointer + offset + BFieldElement::new(3))],
                memory[&(peaks_pointer + offset + BFieldElement::new(4))],
                memory[&(peaks_pointer + offset + BFieldElement::new(5))],
            ]));
        }

        // Run the actual `calculate_new_peaks_from_append` algorithm. This function
        // is inlined here to make it manipulate memory the same way that the TASM code
        // does.
        let auth_path_pointer = BFieldElement::new(65);
        rust_shadowing_helper_functions::list_new(auth_path_pointer, memory);
        rust_shadowing_helper_functions::list_push(peaks_pointer, new_leaf.values(), memory);
        let new_node_index = mmr::shared::data_index_to_node_index(old_leaf_count as u128);
        let (mut right_lineage_count, _height) =
            right_ancestor_count_and_own_height(new_node_index);
        while right_lineage_count != 0 {
            let new_hash = Digest::new(rust_shadowing_helper_functions::list_pop::<DIGEST_LENGTH>(
                peaks_pointer,
                memory,
            ));
            let previous_peak = Digest::new(rust_shadowing_helper_functions::list_pop::<
                DIGEST_LENGTH,
            >(peaks_pointer, memory));
            rust_shadowing_helper_functions::list_push(
                auth_path_pointer,
                previous_peak.values(),
                memory,
            );
            rust_shadowing_helper_functions::list_push(
                peaks_pointer,
                H::hash_pair(&previous_peak, &new_hash).values(),
                memory,
            );
            right_lineage_count -= 1;
        }

        // Pop return values to stack
        stack.push(peaks_pointer);
        stack.push(BFieldElement::new(65)); // Can this be done in a more dynamic way?
    }
}

#[cfg(test)]
mod tests {
    use num::Zero;
    use twenty_first::{
        shared_math::b_field_element::BFieldElement,
        util_types::{
            algebraic_hasher::AlgebraicHasher,
            mmr::{mmr_accumulator::MmrAccumulator, mmr_trait::Mmr},
        },
    };

    use crate::{get_init_tvm_stack, snippet_trait::rust_tasm_equivalence_prop};

    use super::*;

    #[test]
    fn mmra_append_test_empty() {
        type H = RescuePrimeRegular;
        type Mmr = MmrAccumulator<H>;
        let mmra: Mmr = MmrAccumulator::new(vec![]);
        let digest = H::hash(&BFieldElement::zero());
        let expected_final_mmra = MmrAccumulator::new(vec![digest]);
        prop_calculate_new_peaks_from_append(mmra, digest, expected_final_mmra);
    }

    #[test]
    fn mmra_append_test_single() {
        type H = RescuePrimeRegular;
        type Mmr = MmrAccumulator<H>;
        let digest0 = H::hash(&BFieldElement::new(4545));
        let digest1 = H::hash(&BFieldElement::new(12345));
        let mmra: Mmr = MmrAccumulator::new(vec![digest0]);
        let expected_final_mmra = MmrAccumulator::new(vec![digest0, digest1]);
        prop_calculate_new_peaks_from_append(mmra, digest1, expected_final_mmra);
    }

    fn prop_calculate_new_peaks_from_append(
        mut start_mmr: MmrAccumulator<RescuePrimeRegular>,
        new_leaf: Digest,
        expected_mmr: MmrAccumulator<RescuePrimeRegular>,
    ) {
        // We assume that the peaks can safely be stored in memory on address 0
        let peaks_pointer = BFieldElement::zero();

        // BEFORE: _ old_leaf_count_hi old_leaf_count_lo *peaks [digests (new_leaf)]
        // AFTER: _ *new_peaks *auth_path
        let mut init_stack = get_init_tvm_stack();
        let old_leaf_count: u64 = start_mmr.count_leaves() as u64;
        init_stack.push(BFieldElement::new(old_leaf_count >> 32));
        init_stack.push(BFieldElement::new(old_leaf_count & u32::MAX as u64));
        init_stack.push(peaks_pointer);

        // push digests such that element 0 of digest is on top of stack
        for value in new_leaf.values().iter().rev() {
            init_stack.push(*value);
        }

        // Initialize memory
        let mut memory: HashMap<BFieldElement, BFieldElement> = HashMap::default();
        rust_shadowing_helper_functions::list_new(peaks_pointer, &mut memory);
        for peak in start_mmr.get_peaks() {
            rust_shadowing_helper_functions::list_push(peaks_pointer, peak.values(), &mut memory);
        }

        // We assume that the auth paths can safely be stored in memory on address 65
        let auth_paths_pointer = BFieldElement::new(65);
        let mut expected_final_stack = get_init_tvm_stack();
        expected_final_stack.push(peaks_pointer);
        expected_final_stack.push(auth_paths_pointer);

        let _execution_result = rust_tasm_equivalence_prop::<CalculateNewPeaksFromAppend>(
            &init_stack,
            &[],
            &[],
            &mut memory,
            65, // assume that 65 words are allocated in memory when code starts to run
            Some(&expected_final_stack),
        );

        // Find produced MMR
        let peaks_count = memory[&peaks_pointer].value();
        let mut produced_peaks = vec![];
        for i in 0..peaks_count {
            let offset = BFieldElement::new((i as usize * DIGEST_LENGTH) as u64);
            let peak: Digest = Digest::new([
                memory[&(peaks_pointer + offset + BFieldElement::one())],
                memory[&(peaks_pointer + offset + BFieldElement::new(2))],
                memory[&(peaks_pointer + offset + BFieldElement::new(3))],
                memory[&(peaks_pointer + offset + BFieldElement::new(4))],
                memory[&(peaks_pointer + offset + BFieldElement::new(5))],
            ]);
            produced_peaks.push(peak);
        }

        let produced_mmr = MmrAccumulator::<RescuePrimeRegular>::init(
            produced_peaks,
            start_mmr.count_leaves() + 1,
        );

        // Verify that both code paths produce the same MMR
        assert_eq!(expected_mmr, produced_mmr);
    }
}
