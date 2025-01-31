use num::One;
use rand::{random, thread_rng, Rng};
use std::collections::HashMap;
use triton_vm::NonDeterminism;
use twenty_first::shared_math::b_field_element::BFieldElement;

use crate::library::Library;
use crate::rust_shadowing_helper_functions::safe_list::safe_insert_random_list;
use crate::snippet::{DataType, DeprecatedSnippet};
use crate::{get_init_tvm_stack, ExecutionState};

#[derive(Clone, Debug)]
pub struct Length(pub DataType);

impl DeprecatedSnippet for Length {
    fn input_field_names(&self) -> Vec<String> {
        vec!["*list".to_string()]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec!["list_length".to_string()]
    }

    fn input_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::List(Box::new(self.0.clone()))]
    }

    fn output_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U32]
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec![]
    }

    fn gen_input_states(&self) -> Vec<ExecutionState> {
        let mut ret: Vec<ExecutionState> = vec![];
        let mut rng = thread_rng();
        let mut stack = get_init_tvm_stack();
        let list_pointer: BFieldElement = random();
        let capacity: u32 = 100;
        let list_length: usize = rng.gen_range(0..=capacity as usize);
        stack.push(list_pointer);

        let mut memory = HashMap::default();
        safe_insert_random_list(&self.0, list_pointer, capacity, list_length, &mut memory);
        ret.push(ExecutionState::with_stack_and_memory(
            stack.clone(),
            memory,
            0,
        ));
        memory = HashMap::default();
        safe_insert_random_list(&self.0, list_pointer, capacity, list_length, &mut memory);
        ret.push(ExecutionState::with_stack_and_memory(
            stack.clone(),
            memory,
            0,
        ));
        memory = HashMap::default();
        safe_insert_random_list(&self.0, list_pointer, capacity, list_length, &mut memory);
        ret.push(ExecutionState::with_stack_and_memory(
            stack.clone(),
            memory,
            0,
        ));
        memory = HashMap::default();
        safe_insert_random_list(&self.0, list_pointer, capacity, list_length, &mut memory);
        ret.push(ExecutionState::with_stack_and_memory(
            stack.clone(),
            memory,
            0,
        ));
        memory = HashMap::default();
        safe_insert_random_list(&self.0, list_pointer, capacity, list_length, &mut memory);
        ret.push(ExecutionState::with_stack_and_memory(stack, memory, 0));

        ret
    }

    fn stack_diff(&self) -> isize {
        // Consumes a memory address and returns a length in the form of a u32
        0
    }

    fn entrypoint_name(&self) -> String {
        format!(
            "tasm_list_safeimplu32_length___{}",
            self.0.label_friendly_name()
        )
    }

    fn function_code(&self, _library: &mut Library) -> String {
        let entry_point = self.entrypoint_name();
        // Before: _ *list
        // After: _ list_length_u32
        format!(
            "
            {entry_point}:
                read_mem
                swap 1
                pop
                return
                "
        )
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        // Find the list in memory and push its length to the top of the stack
        let list_address = stack.pop().unwrap();
        let list_length = memory[&list_address];
        stack.push(list_length);
    }

    fn common_case_input_state(&self) -> ExecutionState {
        const COMMON_LENGTH: usize = 1 << 5;
        get_benchmark_input_state(COMMON_LENGTH, &self.0)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        const COMMON_LENGTH: usize = 1 << 6;
        get_benchmark_input_state(COMMON_LENGTH, &self.0)
    }
}

fn get_benchmark_input_state(list_length: usize, data_type: &DataType) -> ExecutionState {
    let mut memory = HashMap::default();
    let list_pointer: BFieldElement = BFieldElement::one();

    let capacity = list_length * 2;
    safe_insert_random_list(
        data_type,
        list_pointer,
        capacity as u32,
        list_length,
        &mut memory,
    );

    let mut stack = get_init_tvm_stack();
    stack.push(list_pointer);

    ExecutionState {
        stack,
        std_in: vec![],
        nondeterminism: NonDeterminism::new(vec![]),
        memory,
        words_allocated: 1,
    }
}

#[cfg(test)]
mod tests {
    use num::One;
    use twenty_first::shared_math::b_field_element::BFieldElement;

    use crate::get_init_tvm_stack;
    use crate::test_helpers::{
        test_rust_equivalence_given_input_values_deprecated,
        test_rust_equivalence_multiple_deprecated,
    };

    use super::*;

    #[test]
    fn new_snippet_test_long() {
        test_rust_equivalence_multiple_deprecated(&Length(DataType::Bool), true);
        test_rust_equivalence_multiple_deprecated(&Length(DataType::U32), true);
        test_rust_equivalence_multiple_deprecated(&Length(DataType::U64), true);
        test_rust_equivalence_multiple_deprecated(&Length(DataType::BFE), true);
        test_rust_equivalence_multiple_deprecated(&Length(DataType::XFE), true);
        test_rust_equivalence_multiple_deprecated(&Length(DataType::Digest), true);
    }

    #[test]
    fn list_u32_simple() {
        let expected_end_stack = [get_init_tvm_stack(), vec![BFieldElement::new(42)]].concat();
        prop_length(
            &DataType::U64,
            BFieldElement::one(),
            42,
            Some(&expected_end_stack),
        );

        let expected_end_stack = [get_init_tvm_stack(), vec![BFieldElement::new(588)]].concat();
        prop_length(
            &DataType::XFE,
            BFieldElement::one(),
            588,
            Some(&expected_end_stack),
        );

        let expected_end_stack = [get_init_tvm_stack(), vec![BFieldElement::new(4)]].concat();
        prop_length(
            &DataType::Digest,
            BFieldElement::one(),
            4,
            Some(&expected_end_stack),
        );

        let expected_end_stack = [get_init_tvm_stack(), vec![BFieldElement::new(7)]].concat();
        prop_length(
            &DataType::U32,
            BFieldElement::one(),
            7,
            Some(&expected_end_stack),
        );
    }

    // Note that the *actual list* of length `list_length` is *actually constructed in the VM in this test. So you may not
    // want to exaggerate that number.
    fn prop_length(
        element_type: &DataType,
        list_pointer: BFieldElement,
        list_length: usize,
        expected: Option<&[BFieldElement]>,
    ) {
        let capacity = 1000;
        let mut init_stack = get_init_tvm_stack();
        init_stack.push(list_pointer);

        let mut memory = HashMap::default();

        safe_insert_random_list(
            element_type,
            list_pointer,
            capacity,
            list_length,
            &mut memory,
        );

        test_rust_equivalence_given_input_values_deprecated(
            &Length(DataType::BFE),
            &init_stack,
            &[],
            &mut memory,
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
    fn safe_length_benchmark() {
        bench_and_write(Length(DataType::Digest));
    }
}
