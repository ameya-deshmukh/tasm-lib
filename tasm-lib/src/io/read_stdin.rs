use std::collections::HashMap;

use triton_vm::NonDeterminism;
use twenty_first::shared_math::{b_field_element::BFieldElement, other::random_elements};

use crate::{
    get_init_tvm_stack,
    snippet::{DataType, DeprecatedSnippet},
    ExecutionState,
};

/// Move an element of type `DataType` from standard in to the stack
#[derive(Clone, Debug)]
pub struct ReadStdIn(pub DataType);

impl DeprecatedSnippet for ReadStdIn {
    fn entrypoint_name(&self) -> String {
        format!("tasm_io_read_stdin___{}", self.0.label_friendly_name())
    }

    fn input_field_names(&self) -> Vec<String> {
        vec![]
    }

    fn input_types(&self) -> Vec<DataType> {
        vec![]
    }

    fn output_types(&self) -> Vec<DataType> {
        vec![self.0.clone()]
    }

    fn output_field_names(&self) -> Vec<String> {
        // This function returns element_0 on the top of the stack and the other elements below it. E.g.: _ elem_2 elem_1 elem_0
        let mut ret: Vec<String> = vec![];
        let size = self.0.get_size();
        for i in 0..size {
            ret.push(format!("element_{}", size - 1 - i));
        }

        ret
    }

    fn stack_diff(&self) -> isize {
        self.0.get_size() as isize
    }

    fn function_code(&self, _library: &mut crate::library::Library) -> String {
        let entrypoint = self.entrypoint_name();
        let read_an_element = "read_io\n".repeat(self.0.get_size());

        format!(
            "
            {entrypoint}:
                {read_an_element}
                return
        "
        )
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec!["std input too short".to_string()]
    }

    fn gen_input_states(&self) -> Vec<crate::ExecutionState> {
        let std_in: Vec<BFieldElement> = random_elements(self.0.get_size());
        vec![ExecutionState {
            stack: get_init_tvm_stack(),
            std_in,
            nondeterminism: NonDeterminism::new(vec![]),
            memory: HashMap::default(),
            words_allocated: 0,
        }]
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<twenty_first::shared_math::b_field_element::BFieldElement>,
        std_in: Vec<twenty_first::shared_math::b_field_element::BFieldElement>,
        _secret_in: Vec<twenty_first::shared_math::b_field_element::BFieldElement>,
        _memory: &mut std::collections::HashMap<
            twenty_first::shared_math::b_field_element::BFieldElement,
            twenty_first::shared_math::b_field_element::BFieldElement,
        >,
    ) {
        for elem in std_in.iter().take(self.0.get_size()) {
            stack.push(*elem)
        }
    }

    fn common_case_input_state(&self) -> ExecutionState {
        let mut std_in = vec![];
        std_in.append(&mut random_elements(self.0.get_size()));
        ExecutionState {
            stack: get_init_tvm_stack(),
            std_in,
            nondeterminism: NonDeterminism::new(vec![]),
            memory: HashMap::default(),
            words_allocated: 0,
        }
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        self.common_case_input_state()
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::test_rust_equivalence_multiple_deprecated;

    use super::*;

    #[test]
    fn new_snippet_test() {
        for _ in 0..10 {
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::Bool), true);
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::U32), true);
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::U64), true);
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::U128), true);
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::BFE), true);
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::XFE), true);
            test_rust_equivalence_multiple_deprecated(&ReadStdIn(DataType::Digest), true);
        }
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::snippet_bencher::bench_and_write;

    #[test]
    fn read_stdin_benchmark() {
        bench_and_write(ReadStdIn(DataType::Digest));
    }
}
