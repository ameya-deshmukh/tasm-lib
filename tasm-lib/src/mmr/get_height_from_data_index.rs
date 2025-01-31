use std::collections::HashMap;

use rand::{thread_rng, Rng};
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::other::log_2_floor;

use crate::arithmetic::u64::incr_u64::IncrU64;
use crate::arithmetic::u64::log_2_floor_u64::Log2FloorU64;
use crate::library::Library;
use crate::snippet::{DataType, DeprecatedSnippet};
use crate::{get_init_tvm_stack, ExecutionState};

#[derive(Clone, Debug)]
pub struct GetHeightFromDataIndex;

impl DeprecatedSnippet for GetHeightFromDataIndex {
    fn input_field_names(&self) -> Vec<String> {
        vec!["leaf_index_hi".to_string(), "leaf_index_lo".to_string()]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec!["height".to_string()]
    }

    fn input_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U64]
    }

    fn output_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U32]
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec![]
    }

    fn gen_input_states(&self) -> Vec<crate::ExecutionState> {
        let mut ret: Vec<ExecutionState> = vec![];
        for _ in 0..40 {
            let leaf_index = thread_rng().gen_range(0..u64::MAX / 2);
            ret.push(prepare_state(leaf_index));
        }

        ret
    }

    // Pops `leaf_index` from stack (U32s<2>). Returns height in the form of one u32.
    fn stack_diff(&self) -> isize {
        -1
    }

    fn entrypoint_name(&self) -> String {
        "tasm_mmr_get_height_from_leaf_index".to_string()
    }

    fn function_code(&self, library: &mut Library) -> String {
        let entrypoint = self.entrypoint_name();
        let incr_u64 = library.import(Box::new(IncrU64));
        let log_2_floor_u64 = library.import(Box::new(Log2FloorU64));
        format!(
            "
            // Return the height of the MMR if this data index was the last leaf inserted
            // Before: _ leaf_index_hi leaf_index_lo
            // After: _ height
            {entrypoint}:
                call {incr_u64}
                call {log_2_floor_u64}
                return"
        )
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        _memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        let leaf_index_lo: u32 = stack.pop().unwrap().try_into().unwrap();
        let leaf_index_hi: u32 = stack.pop().unwrap().try_into().unwrap();
        let leaf_index: u64 = (leaf_index_hi as u64) * (1u64 << 32) + leaf_index_lo as u64;
        let height: u32 = log_2_floor(leaf_index as u128 + 1) as u32;
        stack.push(BFieldElement::new(height as u64));
    }

    fn common_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 32) - 1)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 63) - 1)
    }
}

fn prepare_state(leaf_index: u64) -> ExecutionState {
    let mut stack = get_init_tvm_stack();
    let leaf_index_hi = BFieldElement::new(leaf_index >> 32);
    let leaf_index_lo = BFieldElement::new(leaf_index & u32::MAX as u64);
    stack.push(leaf_index_hi);
    stack.push(leaf_index_lo);
    ExecutionState::with_stack(stack)
}

#[cfg(test)]
mod tests {
    use twenty_first::amount::u32s::U32s;
    use twenty_first::shared_math::bfield_codec::BFieldCodec;

    use crate::get_init_tvm_stack;

    use crate::test_helpers::{
        test_rust_equivalence_given_input_values_deprecated,
        test_rust_equivalence_multiple_deprecated,
    };

    use super::*;

    #[test]
    fn get_height_from_data_index_test() {
        test_rust_equivalence_multiple_deprecated(&GetHeightFromDataIndex, true);
    }

    #[test]
    fn get_height_from_leaf_index_test_simple() {
        let mut expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(0));
        prop_get_height_from_leaf_index(0, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(1));
        prop_get_height_from_leaf_index(1, &expected);
        prop_get_height_from_leaf_index(2, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(2));
        prop_get_height_from_leaf_index(3, &expected);
        prop_get_height_from_leaf_index(4, &expected);
        prop_get_height_from_leaf_index(5, &expected);
        prop_get_height_from_leaf_index(6, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(3));
        prop_get_height_from_leaf_index(7, &expected);
        prop_get_height_from_leaf_index(8, &expected);
        prop_get_height_from_leaf_index(9, &expected);
        prop_get_height_from_leaf_index(10, &expected);
        prop_get_height_from_leaf_index(11, &expected);
        prop_get_height_from_leaf_index(12, &expected);
        prop_get_height_from_leaf_index(13, &expected);
        prop_get_height_from_leaf_index(14, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(4));
        prop_get_height_from_leaf_index(15, &expected);
        prop_get_height_from_leaf_index(16, &expected);
        prop_get_height_from_leaf_index(17, &expected);
        prop_get_height_from_leaf_index(18, &expected);
        prop_get_height_from_leaf_index(19, &expected);
        prop_get_height_from_leaf_index(20, &expected);
        prop_get_height_from_leaf_index(21, &expected);
        prop_get_height_from_leaf_index(22, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(31));
        prop_get_height_from_leaf_index(u32::MAX as u64 - 2, &expected);
        prop_get_height_from_leaf_index(u32::MAX as u64 - 1, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(32));
        prop_get_height_from_leaf_index(u32::MAX as u64, &expected);
        prop_get_height_from_leaf_index(u32::MAX as u64 + 1, &expected);
        prop_get_height_from_leaf_index(u32::MAX as u64 + 2, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(44));
        prop_get_height_from_leaf_index((1u64 << 45) - 2, &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(45));
        prop_get_height_from_leaf_index((1u64 << 45) - 1, &expected);
        prop_get_height_from_leaf_index(1u64 << 45, &expected);
        prop_get_height_from_leaf_index((1u64 << 45) + 1, &expected);
        prop_get_height_from_leaf_index((1u64 << 45) + (1 << 40), &expected);

        expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(63));
        prop_get_height_from_leaf_index((1u64 << 63) - 1, &expected);
        prop_get_height_from_leaf_index(1u64 << 63, &expected);
        prop_get_height_from_leaf_index((1u64 << 63) + 1, &expected);
        prop_get_height_from_leaf_index((1u64 << 63) + (1 << 40), &expected);
    }

    fn prop_get_height_from_leaf_index(leaf_index: u64, expected: &[BFieldElement]) {
        let mut init_stack = get_init_tvm_stack();
        let leaf_index_as_u32_2 = U32s::new([
            (leaf_index & 0xFFFFFFFFu32 as u64) as u32,
            (leaf_index >> 32) as u32,
        ]);
        for elem in leaf_index_as_u32_2.encode().into_iter().rev() {
            init_stack.push(elem);
        }

        test_rust_equivalence_given_input_values_deprecated::<GetHeightFromDataIndex>(
            &GetHeightFromDataIndex,
            &init_stack,
            &[],
            &mut HashMap::default(),
            0,
            Some(expected),
        );
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::snippet_bencher::bench_and_write;

    #[test]
    fn get_height_from_data_index_benchmark() {
        bench_and_write(GetHeightFromDataIndex);
    }
}
