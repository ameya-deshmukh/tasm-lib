use std::collections::HashMap;

use itertools::Itertools;
use num_traits::Zero;
use triton_vm::instruction::LabelledInstruction;
use triton_vm::{triton_asm, NonDeterminism};
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::util_types::algebraic_hasher::Domain;

use crate::dyn_malloc::DYN_MALLOC_ADDRESS;
use crate::library::Library;
use crate::snippet::{BasicSnippet, DeprecatedSnippet, RustShadow};
use crate::{
    execute_test, exported_snippets, rust_shadowing_helper_functions, ExecutionState,
    VmHasherState, VmOutputState, DIGEST_LENGTH,
};

#[allow(dead_code)]
pub fn test_rust_equivalence_multiple_deprecated<T: DeprecatedSnippet>(
    snippet_struct: &T,
    export_snippet: bool,
) -> Vec<VmOutputState> {
    // Verify that snippet can be found in `all_snippets`, so that
    // it iss visible to the outside.
    // This call will panic if snippet is not found in that
    // function call. The data type value is a dummy value for all
    // snippets except those that handle lists.
    if export_snippet {
        let looked_up_snippet = exported_snippets::name_to_snippet(&snippet_struct.entrypoint());
        assert_eq!(
            snippet_struct.entrypoint(),
            looked_up_snippet.entrypoint(),
            "Looked up snippet must match self"
        );
    }

    let mut execution_states = snippet_struct.gen_input_states();

    let mut vm_output_states = vec![];
    for execution_state in execution_states.iter_mut() {
        let vm_output_state = test_rust_equivalence_given_execution_state_deprecated::<T>(
            snippet_struct,
            execution_state.clone(),
        );
        vm_output_states.push(vm_output_state);
    }

    vm_output_states
}

#[allow(dead_code)]
pub fn test_rust_equivalence_given_execution_state_deprecated<T: DeprecatedSnippet>(
    snippet_struct: &T,
    mut execution_state: ExecutionState,
) -> VmOutputState {
    let nondeterminism = execution_state.nondeterminism;
    test_rust_equivalence_given_complete_state_deprecated::<T>(
        snippet_struct,
        &execution_state.stack,
        &execution_state.std_in,
        &nondeterminism,
        &mut execution_state.memory,
        execution_state.words_allocated,
        None,
    )
}

#[allow(dead_code)]
pub fn test_rust_equivalence_given_input_values_deprecated<T: DeprecatedSnippet>(
    snippet_struct: &T,
    stack: &[BFieldElement],
    stdin: &[BFieldElement],
    memory: &mut HashMap<BFieldElement, BFieldElement>,
    words_statically_allocated: usize,
    expected_final_stack: Option<&[BFieldElement]>,
) -> VmOutputState {
    let _init_memory = memory.clone();
    let nondeterminism = NonDeterminism::<BFieldElement>::new(vec![]);

    test_rust_equivalence_given_complete_state_deprecated(
        snippet_struct,
        stack,
        stdin,
        &nondeterminism,
        memory,
        words_statically_allocated,
        expected_final_stack,
    )
}

fn link_for_isolated_run_deprecated<T: DeprecatedSnippet>(
    snippet_struct: &T,
    words_statically_allocated: usize,
) -> Vec<LabelledInstruction> {
    let mut snippet_state = Library::with_preallocated_memory(words_statically_allocated);
    let entrypoint = snippet_struct.entrypoint();
    let mut function_body = snippet_struct.function_code(&mut snippet_state);
    function_body.push('\n');
    let library_code = snippet_state.all_imports();

    // The TASM code is always run through a function call, so the 1st instruction
    // is a call to the function in question.
    let code = triton_asm!(
        call {entrypoint}
        halt

        {function_body}
        {&library_code}
    );

    code
}

pub fn link_and_run_tasm_for_test_deprecated<T: DeprecatedSnippet>(
    snippet_struct: &T,
    stack: &mut Vec<BFieldElement>,
    std_in: Vec<BFieldElement>,
    secret_in: Vec<BFieldElement>,
    memory: &mut HashMap<BFieldElement, BFieldElement>,
    words_statically_allocated: usize,
) -> VmOutputState {
    let expected_length_prior: usize = snippet_struct
        .inputs()
        .iter()
        .map(|(x, _n)| x.get_size())
        .sum();
    let expected_length_after: usize = snippet_struct
        .outputs()
        .iter()
        .map(|(x, _n)| x.get_size())
        .sum();
    assert_eq!(
        snippet_struct.stack_diff(),
        (expected_length_after as isize - expected_length_prior as isize),
        "Declared stack diff must match type indicators"
    );

    let code = link_for_isolated_run_deprecated(snippet_struct, words_statically_allocated);

    execute_test(
        &code,
        stack,
        snippet_struct.stack_diff(),
        std_in,
        &mut NonDeterminism::new(secret_in),
        memory,
        Some(words_statically_allocated),
    )
}

#[allow(dead_code)]
#[allow(clippy::ptr_arg)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn test_rust_equivalence_given_complete_state_deprecated<T: DeprecatedSnippet>(
    snippet_struct: &T,
    stack: &[BFieldElement],
    stdin: &[BFieldElement],
    nondeterminism: &NonDeterminism<BFieldElement>,
    memory: &mut HashMap<BFieldElement, BFieldElement>,
    words_statically_allocated: usize,
    expected_final_stack: Option<&[BFieldElement]>,
) -> VmOutputState {
    let init_stack = stack.to_vec();

    let mut rust_memory = memory.clone();
    let mut tasm_memory = memory.clone();
    let mut rust_stack = stack.to_vec();
    let mut tasm_stack = stack.to_vec();

    if words_statically_allocated > 0 {
        rust_shadowing_helper_functions::dyn_malloc::rust_dyn_malloc_initialize(
            &mut rust_memory,
            words_statically_allocated,
        );
    }

    // run rust shadow
    snippet_struct.rust_shadowing(
        &mut rust_stack,
        stdin.to_vec(),
        nondeterminism.individual_tokens.clone(),
        &mut rust_memory,
    );

    // run tvm
    let vm_output_state = link_and_run_tasm_for_test_deprecated(
        snippet_struct,
        &mut tasm_stack,
        stdin.to_vec(),
        nondeterminism.individual_tokens.clone(),
        &mut tasm_memory,
        words_statically_allocated,
    );

    // assert stacks are equal, up to program hash
    let tasm_stack_skip_program_hash = tasm_stack.iter().cloned().skip(DIGEST_LENGTH).collect_vec();
    let rust_stack_skip_program_hash = rust_stack.iter().cloned().skip(DIGEST_LENGTH).collect_vec();
    assert_eq!(
        tasm_stack_skip_program_hash,
        rust_stack_skip_program_hash,
        "Rust code must match TVM for `{}`\n\nTVM: {}\n\nRust: {}. Code was: {}",
        snippet_struct.entrypoint(),
        tasm_stack_skip_program_hash
            .iter()
            .map(|x| x.to_string())
            .collect_vec()
            .join(","),
        rust_stack_skip_program_hash
            .iter()
            .map(|x| x.to_string())
            .collect_vec()
            .join(","),
        snippet_struct.code(&mut Library::new()).iter().join("\n")
    );

    // if expected final stack is given, test against it
    if let Some(expected) = expected_final_stack {
        let expected_final_stack_skip_program_hash =
            expected.iter().skip(DIGEST_LENGTH).cloned().collect_vec();
        assert_eq!(
            tasm_stack_skip_program_hash,
            expected_final_stack_skip_program_hash,
            "TVM must produce expected stack `{}`. \n\nTVM:\n{}\nExpected:\n{}",
            snippet_struct.entrypoint(),
            tasm_stack_skip_program_hash
                .iter()
                .map(|x| x.to_string())
                .collect_vec()
                .join(","),
            expected_final_stack_skip_program_hash
                .iter()
                .map(|x| x.to_string())
                .collect_vec()
                .join(","),
        );
    }

    // Verify that memory behaves as expected, except for the dyn malloc initialization address which
    // is too cumbersome to monitor this way. Its behavior should be tested elsewhere.
    // Alternatively the rust shadowing trait function must take a `Library` argument as input
    // and statically allocate memory from there.
    // TODO: Check if we could perform this check on dyn malloc too
    rust_memory.remove(&BFieldElement::new(DYN_MALLOC_ADDRESS as u64));
    tasm_memory.remove(&BFieldElement::new(DYN_MALLOC_ADDRESS as u64));
    let memory_difference = rust_memory
        .iter()
        .filter(|(k, v)| match tasm_memory.get(*k) {
            Some(b) => *b != **v,
            None => true,
        })
        .chain(
            tasm_memory
                .iter()
                .filter(|(k, v)| match rust_memory.get(*k) {
                    Some(b) => *b != **v,
                    None => true,
                }),
        )
        .collect_vec();
    if rust_memory != tasm_memory {
        let mut tasm_memory = tasm_memory.iter().collect_vec();
        tasm_memory.sort_unstable_by(|&a, &b| a.0.value().partial_cmp(&b.0.value()).unwrap());
        let tasm_mem_str = tasm_memory
            .iter()
            .map(|x| format!("({} => {})", x.0, x.1))
            .collect_vec()
            .join(",");

        let mut rust_memory = rust_memory.iter().collect_vec();
        rust_memory.sort_unstable_by(|&a, &b| a.0.value().partial_cmp(&b.0.value()).unwrap());
        let rust_mem_str = rust_memory
            .iter()
            .map(|x| format!("({} => {})", x.0, x.1))
            .collect_vec()
            .join(",");
        let diff_str = memory_difference
            .iter()
            .map(|x| format!("({} => {})", x.0, x.1))
            .collect_vec()
            .join(",");
        panic!(
            "Memory for both implementations must match after execution.\n\nTVM: {tasm_mem_str}\n\nRust: {rust_mem_str}\n\nDifference: {diff_str}\n\nCode was:\n\n {}",
            snippet_struct.code(&mut Library::new()).iter().join("\n")
        );
    }

    // Write back memory to be able to probe it in individual tests
    *memory = tasm_memory.clone();

    // Verify that stack grows with expected number of elements
    let stack_final = tasm_stack.clone();
    let observed_stack_growth: isize = stack_final.len() as isize - init_stack.len() as isize;
    let expected_stack_growth: isize = snippet_struct.output_field_names().len() as isize
        - snippet_struct.input_field_names().len() as isize;
    assert_eq!(
        expected_stack_growth,
        observed_stack_growth,
        "Stack must pop and push expected number of elements. Got input: {}\nGot output: {}",
        init_stack.iter().map(|x| x.to_string()).join(","),
        stack_final.iter().map(|x| x.to_string()).join(",")
    );

    vm_output_state
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use rand::random;
    use triton_vm::{BFieldElement, NonDeterminism};
    use twenty_first::shared_math::tip5::DIGEST_LENGTH;

    use crate::{get_init_tvm_stack, hashing::sample_indices::SampleIndices, list::ListType};

    use super::test_rust_equivalence_given_complete_state_deprecated;

    /// TIP6 sets the bottom of the stack to the program hash. While testing Snippets,
    /// which are not standalone programs and therefore do not come with a well defined
    /// program hash, we want to verify that the tasm and rust stacks are identical up
    /// to these first five elements. This unit test tests this.
    #[test]
    fn test_program_hash_ignored() {
        let snippet_struct = SampleIndices {
            list_type: ListType::Safe,
        };
        let mut stack = get_init_tvm_stack();
        stack.push(BFieldElement::new(45u64));
        stack.push(BFieldElement::new(1u64 << 12));

        let mut init_memory = HashMap::new();
        let mut tasm_stack = stack.to_vec();
        for item in tasm_stack.iter_mut().take(DIGEST_LENGTH) {
            *item = random();
        }

        test_rust_equivalence_given_complete_state_deprecated(
            &snippet_struct,
            &stack,
            &[],
            &NonDeterminism::new(vec![]),
            &mut init_memory,
            1,
            None,
        );
    }
}

pub fn rust_final_state<T: RustShadow>(
    shadowed_snippet: &T,
    stack: &[BFieldElement],
    stdin: &[BFieldElement],
    nondeterminism: &NonDeterminism<BFieldElement>,
    memory: &HashMap<BFieldElement, BFieldElement>,
    sponge_state: &VmHasherState,
    words_statically_allocated: usize,
) -> VmOutputState {
    let mut rust_memory = memory.clone();
    let mut rust_stack = stack.to_vec();
    let mut rust_sponge = sponge_state.clone();

    // allocate memory, if necessary
    if words_statically_allocated > 0 && memory.get(&BFieldElement::zero()).is_none() {
        rust_shadowing_helper_functions::dyn_malloc::rust_dyn_malloc_initialize(
            &mut rust_memory,
            words_statically_allocated,
        );
    }

    // run rust shadow
    let output = shadowed_snippet.rust_shadow_wrapper(
        stdin,
        nondeterminism,
        &mut rust_stack,
        &mut rust_memory,
        &mut rust_sponge,
    );

    VmOutputState {
        output,
        final_stack: rust_stack,
        final_ram: rust_memory,
        final_sponge_state: rust_sponge,
    }
}

pub fn tasm_final_state<T: RustShadow>(
    shadowed_snippet: &T,
    stack: &[BFieldElement],
    stdin: &[BFieldElement],
    nondeterminism: &NonDeterminism<BFieldElement>,
    memory: &HashMap<BFieldElement, BFieldElement>,
    _sponge_state: &VmHasherState,
    words_statically_allocated: usize,
) -> VmOutputState {
    // allocate memory, if necessary
    let mut tasm_memory = memory.clone();
    if words_statically_allocated > 0 && memory.get(&BFieldElement::zero()).is_none() {
        rust_shadowing_helper_functions::dyn_malloc::rust_dyn_malloc_initialize(
            &mut tasm_memory,
            words_statically_allocated,
        );
    }

    // run tvm
    link_and_run_tasm_for_test(
        shadowed_snippet,
        &mut stack.to_vec(),
        stdin.to_vec(),
        &mut nondeterminism.clone(),
        &mut tasm_memory,
        words_statically_allocated,
    )
}

pub fn verify_stack_equivalence(a: &[BFieldElement], b: &[BFieldElement]) {
    // assert stacks are equal, up to program hash
    let a_skip_program_hash = a.iter().cloned().skip(DIGEST_LENGTH).collect_vec();
    let b_skip_program_hash = b.iter().cloned().skip(DIGEST_LENGTH).collect_vec();
    assert_eq!(
        a_skip_program_hash,
        b_skip_program_hash,
        "A stack must match B stack\n\nA: {}\n\nB: {}",
        a_skip_program_hash
            .iter()
            .map(|x| x.to_string())
            .collect_vec()
            .join(","),
        b_skip_program_hash
            .iter()
            .map(|x| x.to_string())
            .collect_vec()
            .join(","),
    );
}

pub fn verify_memory_equivalence(
    a_memory: &HashMap<BFieldElement, BFieldElement>,
    b_memory: &HashMap<BFieldElement, BFieldElement>,
) {
    // verify equivalence of memory up to the value of dynamic allocator
    let memory_difference = b_memory
        .iter()
        .filter(|(k, v)| match a_memory.get(*k) {
            Some(b) => *b != **v,
            None => true,
        })
        .chain(a_memory.iter().filter(|(k, v)| match b_memory.get(*k) {
            Some(b) => *b != **v,
            None => true,
        }))
        .collect_vec();
    if memory_difference
        .iter()
        .any(|(k, _v)| **k != BFieldElement::new(DYN_MALLOC_ADDRESS as u64))
    {
        let mut a_memory_ = a_memory.iter().collect_vec();
        a_memory_.sort_unstable_by(|&a, &b| a.0.value().partial_cmp(&b.0.value()).unwrap());
        let a_mem_str = a_memory_
            .iter()
            .map(|x| format!("({} => {})", x.0, x.1))
            .collect_vec()
            .join(",");

        let mut b_memory_ = b_memory.iter().collect_vec();
        b_memory_.sort_unstable_by(|&a, &b| a.0.value().partial_cmp(&b.0.value()).unwrap());
        let b_mem_str = b_memory_
            .iter()
            .map(|x| format!("({} => {})", x.0, x.1))
            .collect_vec()
            .join(",");
        let diff_str = memory_difference
            .iter()
            .map(|x| format!("({} => {})", x.0, x.1))
            .collect_vec()
            .join(",");
        panic!(
            "Memory for both implementations must match after execution.\n\nA: {a_mem_str}\n\nB: {b_mem_str}\n\nDifference: {diff_str}\n\n",
        );
    }
}

pub fn verify_hasher_state_equivalence(a: VmOutputState, b: VmOutputState) {
    assert_eq!(a.final_sponge_state.state, b.final_sponge_state.state);
}

pub fn verify_stack_growth<T: RustShadow>(
    shadowed_snippet: &T,
    initial_stack: &[BFieldElement],
    final_stack: &[BFieldElement],
) {
    let observed_stack_growth: isize = final_stack.len() as isize - initial_stack.len() as isize;
    let expected_stack_growth: isize = shadowed_snippet.inner().borrow().stack_diff();
    assert_eq!(
        expected_stack_growth,
        observed_stack_growth,
        "Stack must pop and push expected number of elements. Got input: {}\nGot output: {}",
        initial_stack.iter().map(|x| x.to_string()).join(","),
        final_stack.iter().map(|x| x.to_string()).join(",")
    );
}

#[allow(dead_code)]
#[allow(clippy::ptr_arg)]
#[allow(clippy::too_many_arguments)]
pub fn test_rust_equivalence_given_complete_state<T: RustShadow>(
    shadowed_snippet: &T,
    stack: &[BFieldElement],
    stdin: &[BFieldElement],
    nondeterminism: &NonDeterminism<BFieldElement>,
    memory: &HashMap<BFieldElement, BFieldElement>,
    sponge_state: &VmHasherState,
    words_statically_allocated: usize,
    expected_final_stack: Option<&[BFieldElement]>,
) -> VmOutputState {
    let init_stack = stack.to_vec();

    let rust = rust_final_state(
        shadowed_snippet,
        stack,
        stdin,
        nondeterminism,
        memory,
        sponge_state,
        words_statically_allocated,
    );

    // run tvm
    let tasm = tasm_final_state(
        shadowed_snippet,
        stack,
        stdin,
        nondeterminism,
        memory,
        sponge_state,
        words_statically_allocated,
    );

    assert_eq!(
        rust.output, tasm.output,
        "Rust shadowing and VM std out must agree"
    );

    verify_stack_equivalence(&rust.final_stack, &tasm.final_stack);
    if let Some(expected) = expected_final_stack {
        verify_stack_equivalence(expected, &rust.final_stack);
    }
    verify_memory_equivalence(&rust.final_ram, &tasm.final_ram);
    verify_stack_growth(shadowed_snippet, &init_stack, &tasm.final_stack);

    tasm
}

pub fn link_and_run_tasm_for_test<T: RustShadow>(
    snippet_struct: &T,
    stack: &mut Vec<BFieldElement>,
    std_in: Vec<BFieldElement>,
    nondeterminism: &mut NonDeterminism<BFieldElement>,
    memory: &mut HashMap<BFieldElement, BFieldElement>,
    words_statically_allocated: usize,
) -> VmOutputState {
    let words_statically_allocated = if let Some(allocator) = memory.get(&BFieldElement::zero()) {
        allocator.value() as usize
    } else {
        words_statically_allocated
    };

    let code = link_for_isolated_run(snippet_struct, words_statically_allocated);

    execute_test(
        &code,
        stack,
        snippet_struct.inner().borrow().stack_diff(),
        std_in,
        nondeterminism,
        memory,
        Some(words_statically_allocated),
    )
}

fn link_for_isolated_run<T: RustShadow>(
    snippet_struct: &T,
    words_statically_allocated: usize,
) -> Vec<LabelledInstruction> {
    println!("linking with preallocated memory ... number of statically allocated words: {words_statically_allocated}");
    let mut snippet_state = Library::with_preallocated_memory(words_statically_allocated);
    let entrypoint = snippet_struct.inner().borrow().entrypoint();
    let function_body = snippet_struct.inner().borrow().code(&mut snippet_state);
    let library_code = snippet_state.all_imports();

    // The TASM code is always run through a function call, so the 1st instruction
    // is a call to the function in question.
    let code = triton_asm!(
        call {entrypoint}
        halt

        {&function_body}
        {&library_code}
    );

    code
}

#[allow(dead_code)]
pub fn test_rust_equivalence_given_execution_state<T: BasicSnippet + RustShadow>(
    snippet_struct: &T,
    execution_state: ExecutionState,
) -> VmOutputState {
    let nondeterminism = execution_state.nondeterminism;
    test_rust_equivalence_given_complete_state::<T>(
        snippet_struct,
        &execution_state.stack,
        &execution_state.std_in,
        &nondeterminism,
        &execution_state.memory,
        &VmHasherState::new(Domain::FixedLength),
        execution_state.words_allocated,
        None,
    )
}
