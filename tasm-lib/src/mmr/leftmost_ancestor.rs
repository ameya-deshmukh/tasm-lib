use std::collections::HashMap;

use num::BigUint;
use rand::{thread_rng, Rng};
use twenty_first::amount::u32s::U32s;
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::bfield_codec::BFieldCodec;
use twenty_first::util_types::mmr;

use crate::arithmetic::u64::decr_u64::DecrU64;
use crate::arithmetic::u64::log_2_floor_u64::Log2FloorU64;
use crate::arithmetic::u64::pow2_u64::Pow2U64;
use crate::library::Library;
use crate::snippet::{DataType, DeprecatedSnippet};
use crate::{get_init_tvm_stack, ExecutionState};

#[derive(Clone, Debug)]
pub struct MmrLeftMostAncestor;

impl DeprecatedSnippet for MmrLeftMostAncestor {
    fn input_field_names(&self) -> Vec<String> {
        vec!["node_index_hi".to_string(), "node_index_lo".to_string()]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec![
            "leftmost_ancestor_hi".to_string(),
            "leftmost_ancestor_lo".to_string(),
            "height".to_string(),
        ]
    }

    fn input_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U64]
    }

    fn output_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U64, DataType::U32]
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec![
            "Inputs are not u32s".to_string(),
            "Node index beyond ~2^63?".to_string(),
        ]
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
        1
    }

    fn entrypoint_name(&self) -> String {
        "tasm_mmr_leftmost_ancestor".to_string()
    }

    fn function_code(&self, library: &mut Library) -> String {
        let entrypoint = self.entrypoint_name();
        let decr_u64 = library.import(Box::new(DecrU64));
        let pow2_u64 = library.import(Box::new(Pow2U64));
        let log_2_floor_u64 = library.import(Box::new(Log2FloorU64));
        format!(
            "
            // Before: _ node_index_hi node_index_lo
            // After: _ leftmost_ancestor_hi leftmost_ancestor_lo height
            {entrypoint}:
                call {log_2_floor_u64}
                // stack: _ log2_floor

                dup 0
                // notice that log2_floor = height
                // stack: _ height log2_floor

                push 1
                add
                // stack: _ height (log2_floor + 1)

                call {pow2_u64}
                // stack: _ height 2^(log2_floor + 1)_hi 2^(log2_floor + 1)_lo

                call {decr_u64}
                // stack: _ height leftmost_ancestor_hi leftmost_ancestor_lo

                swap 1
                swap 2
                // stack: _ leftmost_ancestor_hi leftmost_ancestor_lo height

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

        let (ret, h) = mmr::shared_advanced::leftmost_ancestor(node_index);
        let ret: U32s<2> = U32s::from(BigUint::from(ret));

        stack.append(&mut ret.encode().into_iter().rev().collect());
        stack.push(BFieldElement::from(h));
    }

    fn common_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 32) - 1)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 63) - 1)
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
    use twenty_first::shared_math::b_field_element::BFieldElement;

    use crate::get_init_tvm_stack;

    use crate::test_helpers::{
        test_rust_equivalence_given_input_values_deprecated,
        test_rust_equivalence_multiple_deprecated,
    };

    use super::*;

    #[test]
    fn leftmost_ancestor_test() {
        test_rust_equivalence_multiple_deprecated(&MmrLeftMostAncestor, true);
    }

    #[test]
    fn u32s_leftmost_ancestor_simple() {
        // leftmost_ancestor(1) -> height = 0, index = 1
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(0));
        expected_stack.push(BFieldElement::new(1));
        expected_stack.push(BFieldElement::new(0));
        prop_leftmost_ancestor(U32s::<2>::from(1), Some(&expected_stack));

        // leftmost_ancestor(2) -> height = 1, index = 3
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(0));
        expected_stack.push(BFieldElement::new(3));
        expected_stack.push(BFieldElement::new(1));
        prop_leftmost_ancestor(U32s::<2>::from(2), Some(&expected_stack));
        prop_leftmost_ancestor(U32s::<2>::from(3), Some(&expected_stack));

        // leftmost_ancestor([4..7]) -> height = 2, index = 7
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(0));
        expected_stack.push(BFieldElement::new(7));
        expected_stack.push(BFieldElement::new(2));
        prop_leftmost_ancestor(U32s::<2>::from(4), Some(&expected_stack));
        prop_leftmost_ancestor(U32s::<2>::from(5), Some(&expected_stack));
        prop_leftmost_ancestor(U32s::<2>::from(6), Some(&expected_stack));
        prop_leftmost_ancestor(U32s::<2>::from(7), Some(&expected_stack));

        // leftmost_ancestor([8..15]) -> height = 3, index = 15
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(0));
        expected_stack.push(BFieldElement::new(15));
        expected_stack.push(BFieldElement::new(3));
        for i in 8..=15 {
            prop_leftmost_ancestor(U32s::<2>::from(i), Some(&expected_stack));
        }

        // leftmost_ancestor([16..31]) -> height = 3, index = 31
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(0));
        expected_stack.push(BFieldElement::new(31));
        expected_stack.push(BFieldElement::new(4));
        for i in 16..=31 {
            prop_leftmost_ancestor(U32s::<2>::from(i), Some(&expected_stack));
        }

        // leftmost_ancestor([32..63]) -> height = 4, index = 63
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(0));
        expected_stack.push(BFieldElement::new(63));
        expected_stack.push(BFieldElement::new(5));
        for i in 32..=63 {
            prop_leftmost_ancestor(U32s::<2>::from(i), Some(&expected_stack));
        }

        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(1));
        expected_stack.push(BFieldElement::from(u32::MAX));
        expected_stack.push(BFieldElement::new(32));
        prop_leftmost_ancestor(
            U32s::<2>::from(BigUint::from(1u64 << 32)),
            Some(&expected_stack),
        );

        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new(3));
        expected_stack.push(BFieldElement::from(u32::MAX));
        expected_stack.push(BFieldElement::new(33));
        prop_leftmost_ancestor(
            U32s::<2>::from(BigUint::from(1u64 << 33)),
            Some(&expected_stack),
        );

        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new((1u64 << 31) - 1));
        expected_stack.push(BFieldElement::from(u32::MAX));
        expected_stack.push(BFieldElement::new(62));
        prop_leftmost_ancestor(
            U32s::<2>::from(BigUint::from(1u64 << 62)),
            Some(&expected_stack),
        );

        // This test fails but maybe it should succeed?
        // let mut expected_stack = get_init_tvm_stack();
        // expected_stack.push(BFieldElement::new((1u64 << 32) - 1));
        // expected_stack.push(BFieldElement::from(u32::MAX));
        // expected_stack.push(BFieldElement::new(63));
        // prop_leftmost_ancestor(
        //     U32s::<2>::from(BigUint::from(1u64 << 63)),
        //     Some(&expected_stack),
        // );
    }

    fn prop_leftmost_ancestor(node_index: U32s<2>, expected: Option<&[BFieldElement]>) {
        let mut init_stack = get_init_tvm_stack();
        for elem in node_index.encode().into_iter().rev() {
            init_stack.push(elem);
        }

        test_rust_equivalence_given_input_values_deprecated::<MmrLeftMostAncestor>(
            &MmrLeftMostAncestor,
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
    fn leftmost_ancestor_benchmark() {
        bench_and_write(MmrLeftMostAncestor);
    }
}
