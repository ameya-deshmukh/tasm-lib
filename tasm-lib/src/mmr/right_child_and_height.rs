use std::collections::HashMap;

use num::{One, Zero};
use rand::{thread_rng, Rng};
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::util_types::mmr;

use crate::arithmetic::u64::eq_u64::EqU64;
use crate::arithmetic::u64::lt_u64::LtU64;
use crate::library::Library;
use crate::snippet::{DataType, DeprecatedSnippet};
use crate::{get_init_tvm_stack, ExecutionState};

use super::left_child::MmrLeftChild;
use super::leftmost_ancestor::MmrLeftMostAncestor;
use super::right_child::MmrRightChild;

// You probably don't want to use this but a right lineage count function instead
#[derive(Clone, Debug)]
pub struct MmrRightChildAndHeight;

impl DeprecatedSnippet for MmrRightChildAndHeight {
    fn input_field_names(&self) -> Vec<String> {
        vec!["node_index_hi".to_string(), "node_index_lo".to_string()]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec!["is_right_child".to_string(), "height".to_string()]
    }

    fn input_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U64]
    }

    fn output_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::Bool, DataType::U32]
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec!["Node index exceeds 2^63?".to_string()]
    }

    fn gen_input_states(&self) -> Vec<crate::ExecutionState> {
        let mut ret: Vec<ExecutionState> = vec![];
        for _ in 0..10 {
            let node_index = thread_rng().gen_range(0..u64::MAX / 2);
            ret.push(prepare_state(node_index));
        }

        ret
    }

    fn stack_diff(&self) -> isize {
        0
    }

    fn entrypoint_name(&self) -> String {
        "tasm_mmr_right_child_and_height".to_string()
    }

    fn function_code(&self, library: &mut Library) -> String {
        let entrypoint = self.entrypoint_name();
        let eq_u64 = library.import(Box::new(EqU64));
        let lt_u64 = library.import(Box::new(LtU64));
        let left_child = library.import(Box::new(MmrLeftChild));
        let right_child = library.import(Box::new(MmrRightChild));
        let leftmost_ancestor = library.import(Box::new(MmrLeftMostAncestor));

        format!(
            "
            // Before: _ ni_hi ni_lo
            // After: _ is_right_child height
            {entrypoint}:
                // Get leftmost ancestor and its height on top of stack
                push 0 // is `is_r` onto stack
                dup 2
                dup 2
                call {leftmost_ancestor}
                // stack: _ ni_hi ni_lo is_r c_hi c_lo height

                swap 2
                swap 1
                // stack: _ ni_hi ni_lo is_r height c_hi c_lo
                call {entrypoint}_loop
                // Stack: ni_hi ni_lo is_r height c_hi c_lo
                pop
                pop
                swap 2
                pop
                swap 2
                pop

                // Stack: _ is_r height
                return

            // Stack start and end:
            // _ ni_hi ni_lo is_r height c_hi c_lo
            {entrypoint}_loop:
                dup 5
                dup 5
                dup 3
                dup 3
                call {eq_u64}
                // Stack: _ ni_hi ni_lo is_r height c_hi c_lo (c == ni)
                skiz return

                // Stack: ni_hi ni_lo is_r height c_hi c_lo
                dup 1
                dup 1
                dup 4
                // Stack: ni_hi ni_lo is_r height c_hi c_lo c_hi c_lo height

                call {left_child}
                // Stack: ni_hi ni_lo is_r height c_hi c_lo lc_hi lc_lo

                dup 7 dup 7
                // Stack: ni_hi ni_lo is_r height c_hi c_lo lc_hi lc_lo ni_hi ni_lo
                swap 2
                swap 1
                swap 3
                swap 1
                // Stack: ni_hi ni_lo is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo

                call {lt_u64}


                // Stack: ni_hi ni_lo prev_is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo is_r
                push 1
                dup 1
                // Stack: _ ni_hi ni_lo prev_is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo is_r 1 is_r

                skiz call {entrypoint}_branch_then
                skiz call {entrypoint}_branch_else
                // Stack: _ ni_hi ni_lo is_r height c_hi c_lo

                // Decrement height by one
                swap 2
                push -1
                add
                swap 2

                // Stack: _ ni_hi ni_lo is_r (height - 1) c_hi c_lo

                recurse

            {entrypoint}_branch_then:
                // purpose: Set candidate to right child
                // Stack: _ ni_hi ni_lo prev_is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo is_r 1
                pop
                swap 8
                pop
                // Stack: ni_hi ni_lo is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo

                pop pop pop pop
                // Stack: ni_hi ni_lo is_r height c_hi c_lo

                call {right_child}
                // Stack: ni_hi ni_lo is_r height rc_hi rc_lo
                // Stack: ni_hi ni_lo is_r height c_hi c_lo (after rename)

                push 0
                // End stack: ni_hi ni_lo is_r height c_hi c_lo 0

                return

            {entrypoint}_branch_else:
                // purpose: Set candidate to left child
                // Stack: _ ni_hi ni_lo prev_is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo is_r

                swap 8
                pop
                // Stack: ni_hi ni_lo is_r height c_hi c_lo ni_hi ni_lo lc_hi lc_lo

                swap 4 pop swap 4 pop
                // Stack: ni_hi ni_lo is_r height lc_hi lc_lo ni_hi ni_lo

                pop pop
                // Stack: ni_hi ni_lo is_r height lc_hi lc_lo

                // Stack: ni_hi ni_lo is_r height lc_hi lc_lo
                // End stack: ni_hi ni_lo is_r height c_hi c_lo (after rename)
                return

            "
        )
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        _memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        let node_index_lo: u32 = stack.pop().unwrap().try_into().unwrap();
        let node_index_hi: u32 = stack.pop().unwrap().try_into().unwrap();
        let node_index: u64 = (node_index_hi as u64) * (1u64 << 32) + node_index_lo as u64;

        // FIXME: We probably want to remove `right_child_and_height`, but we're interested
        // in seeing the relative clock cycle count after introducing the U32 Table.
        let (ret, height) = mmr::shared_advanced::right_lineage_length_and_own_height(node_index);
        stack.push(if ret != 0 {
            BFieldElement::one()
        } else {
            BFieldElement::zero()
        });

        stack.push(BFieldElement::new(height as u64));
    }

    fn common_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 32) + 1)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 62) + 1)
    }
}

fn prepare_state(node_index: u64) -> ExecutionState {
    let mut stack = get_init_tvm_stack();
    let node_index_hi = BFieldElement::new(node_index >> 32);
    let node_index_lo = BFieldElement::new(node_index & u32::MAX as u64);
    stack.push(node_index_hi);
    stack.push(node_index_lo);
    ExecutionState::with_stack(stack)
}

#[cfg(test)]
mod tests {
    use twenty_first::amount::u32s::U32s;
    use twenty_first::shared_math::b_field_element::BFieldElement;
    use twenty_first::shared_math::bfield_codec::BFieldCodec;

    use crate::get_init_tvm_stack;

    use crate::test_helpers::{
        test_rust_equivalence_given_input_values_deprecated,
        test_rust_equivalence_multiple_deprecated,
    };

    use super::*;

    #[test]
    fn right_child_and_height_test() {
        test_rust_equivalence_multiple_deprecated(&MmrRightChildAndHeight, true);
    }

    #[test]
    fn right_child_and_height_node_index_equal_leftmost_ancestor() {
        // All should return (false, height) as leftmost ancestors are always left-children.
        let expected_end_stack = [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat();
        prop_right_child_and_height(
            U32s::new([1, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::zero()]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([3, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::one()]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([7, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::new(2)]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([15, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::new(3)]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([31, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::new(4)]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([63, 0]),
            Some(&[expected_end_stack, vec![BFieldElement::new(5)]].concat()),
        );
    }

    #[test]
    fn right_child_and_height_node_index_any() {
        prop_right_child_and_height(
            U32s::new([1, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([2, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::one()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([3, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::one()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([4, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([5, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::one()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([6, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::one()]].concat(),
                    vec![BFieldElement::one()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([7, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::new(2)],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([8, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([14, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::one()]].concat(),
                    vec![BFieldElement::new(2)],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([15, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::new(3)],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([16, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([17, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::one()]].concat(),
                    vec![BFieldElement::zero()],
                ]
                .concat(),
            ),
        );
        println!("18");
        prop_right_child_and_height(
            U32s::new([18, 0]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::one()],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([u32::MAX - 1, u32::MAX / 2]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::one()]].concat(),
                    vec![BFieldElement::new(61)],
                ]
                .concat(),
            ),
        );
        prop_right_child_and_height(
            U32s::new([u32::MAX, u32::MAX / 2]),
            Some(
                &[
                    [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat(),
                    vec![BFieldElement::new(62)],
                ]
                .concat(),
            ),
        );
    }

    #[test]
    fn right_child_and_height_node_is_left_child() {
        // All should return (false, height) as leftmost ancestors are always left-children.
        let expected_end_stack = [get_init_tvm_stack(), vec![BFieldElement::zero()]].concat();
        prop_right_child_and_height(
            U32s::new([1, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::zero()]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([3, 0]),
            Some(&[expected_end_stack.clone(), vec![BFieldElement::one()]].concat()),
        );
        prop_right_child_and_height(
            U32s::new([4, 0]),
            Some(&[expected_end_stack, vec![BFieldElement::zero()]].concat()),
        );
    }

    fn prop_right_child_and_height(node_index: U32s<2>, expected: Option<&[BFieldElement]>) {
        let mut init_stack = get_init_tvm_stack();
        for elem in node_index.encode().into_iter().rev() {
            init_stack.push(elem);
        }

        test_rust_equivalence_given_input_values_deprecated::<MmrRightChildAndHeight>(
            &MmrRightChildAndHeight,
            &init_stack,
            &[],
            &mut HashMap::default(),
            0,
            expected,
        );
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::snippet_bencher::bench_and_write;

    #[test]
    fn right_child_and_height_benchmark() {
        bench_and_write(MmrRightChildAndHeight);
    }
}
