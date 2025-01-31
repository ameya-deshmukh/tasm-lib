use std::collections::HashMap;

use num::{One, Zero};
use rand::Rng;
use triton_vm::{instruction::LabelledInstruction, triton_instr};
use twenty_first::shared_math::b_field_element::{BFieldElement, BFIELD_ZERO};

pub const DYN_MALLOC_ADDRESS: u32 = 0;

use crate::{
    get_init_tvm_stack,
    library::Library,
    snippet::{DataType, DeprecatedSnippet},
    ExecutionState,
};

#[derive(Clone, Debug)]
pub struct DynMalloc;

impl DynMalloc {
    pub fn get_initialization_code(malloc_init_value: u32) -> Vec<LabelledInstruction> {
        let mut ret = Vec::default();

        if malloc_init_value > 0 {
            ret.push(triton_instr!(push DYN_MALLOC_ADDRESS as u64));
            ret.push(triton_instr!(push malloc_init_value as u64));
            ret.push(triton_instr!(write_mem));
            ret.push(triton_instr!(pop));
        }

        ret
    }
}

impl DeprecatedSnippet for DynMalloc {
    fn entrypoint_name(&self) -> String {
        "tasm_memory_dyn_malloc".to_string()
    }

    fn input_field_names(&self) -> Vec<String> {
        vec!["size".to_string()]
    }

    fn input_types(&self) -> Vec<DataType> {
        vec![DataType::U32]
    }

    fn output_types(&self) -> Vec<DataType> {
        vec![DataType::U32]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec!["*addr".to_string()]
    }

    fn stack_diff(&self) -> isize {
        0
    }

    fn function_code(&self, _library: &mut Library) -> String {
        let entrypoint = self.entrypoint_name();
        format!(
            "
            // Return a pointer to a free address and allocate `size` words for this pointer

            // Before: _ size
            // After: _ *next_addr
            {entrypoint}:
                push {DYN_MALLOC_ADDRESS}  // _ size *free_pointer
                read_mem                   // _ size *free_pointer *next_addr'

                // add 1 iff `next_addr` was 0, i.e. uninitialized.
                dup 0                      // _ size *free_pointer *next_addr' *next_addr'
                push 0                     // _ size *free_pointer *next_addr' *next_addr' 0
                eq                         // _ size *free_pointer *next_addr' (*next_addr' == 0)
                add                        // _ size *free_pointer *next_addr

                dup 0                      // _ size *free_pointer *next_addr *next_addr
                dup 3                      // _ size *free_pointer *next_addr *next_addr size

                // Ensure that `size` does not exceed 2^32
                split
                swap 1
                push 0
                eq
                assert

                add                        // _ size *free_pointer *next_addr *(next_addr + size)

                // Ensure that no more than 2^32 words are allocated, because I don't want a wrap-around
                // in the address space
                split
                swap 1
                push 0
                eq
                assert

                swap 1                     // _ size *free_pointer *(next_addr + size) *next_addr
                swap 3                     // _ *next_addr *free_pointer *(next_addr + size) size
                pop                        // _ *next_addr *free_pointer *(next_addr + size)
                write_mem
                pop                        // _ next_addr
                return
            "
        )
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec![
            "Caller attempts to allocate more than 2^32 words".to_owned(),
            "More than 2^32 words allocated to memory".to_owned(),
        ]
    }

    fn gen_input_states(&self) -> Vec<ExecutionState> {
        let mut rng = rand::thread_rng();

        let mut stack = get_init_tvm_stack();
        stack.push(BFieldElement::new(rng.gen_range(0..10_000)));

        let static_allocation_size = rng.gen_range(0..10_000);
        let memory = HashMap::<BFieldElement, BFieldElement>::new();

        let ret: Vec<ExecutionState> = vec![
            ExecutionState::with_stack_and_memory(stack, memory, static_allocation_size),
            ExecutionState::with_stack(get_init_tvm_stack()),
        ];

        ret
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        let allocator_addr = BFIELD_ZERO;
        let used_memory = memory
            .entry(allocator_addr)
            .and_modify(|e| {
                *e = if e.is_zero() {
                    BFieldElement::one()
                } else {
                    *e
                }
            })
            .or_insert_with(BFieldElement::one);

        let size = stack.pop().unwrap();
        assert!(size.value() < (1u64 << 32));

        let next_addr = *used_memory;

        stack.push(next_addr);
        *used_memory += size;

        assert!(used_memory.value() < (1u64 << 32));
    }

    fn common_case_input_state(&self) -> ExecutionState {
        let mut init_stack = get_init_tvm_stack();
        init_stack.push(BFieldElement::new(10));
        ExecutionState::with_stack(init_stack)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        let mut init_stack = get_init_tvm_stack();
        init_stack.push(BFieldElement::new(1 << 31));
        ExecutionState::with_stack(init_stack)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::test_rust_equivalence_multiple_deprecated;

    use super::*;

    #[test]
    fn sane_address_chosen_for_dyn_malloc() {
        // It's probably a really bad idea to use any other value than 0.
        assert_eq!(0, DYN_MALLOC_ADDRESS);
    }

    #[test]
    fn dyn_malloc_test() {
        test_rust_equivalence_multiple_deprecated(&DynMalloc, true);
    }

    #[test]
    fn unit_test() {
        let mut init_stack = get_init_tvm_stack();
        init_stack.push(BFieldElement::new(10));
        let mut empty_memory_state = ExecutionState::with_stack(init_stack.clone());
        DynMalloc.link_and_run_tasm_from_state_for_test(&mut empty_memory_state);
        assert!(empty_memory_state.stack.pop().unwrap().is_one());

        let mut non_empty_memory_state =
            ExecutionState::with_stack_and_memory(init_stack, HashMap::default(), 100);
        DynMalloc.link_and_run_tasm_from_state_for_test(&mut non_empty_memory_state);
        assert_eq!(100, non_empty_memory_state.stack.pop().unwrap().value());
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::snippet_bencher::bench_and_write;

    #[test]
    fn dyn_malloc_benchmark() {
        bench_and_write(DynMalloc);
    }
}
