use std::collections::HashMap;

use rand::RngCore;
use twenty_first::shared_math::b_field_element::BFieldElement;

use crate::library::Library;
use crate::snippet::{NewSnippet, Snippet};
use crate::{get_init_tvm_stack, push_hashable, ExecutionState};

pub struct U32IsOdd();

impl NewSnippet for U32IsOdd {
    fn inputs() -> Vec<&'static str> {
        vec!["value"]
    }

    fn outputs() -> Vec<&'static str> {
        vec!["value % 2"]
    }

    fn crash_conditions() -> Vec<&'static str> {
        vec!["if `value` is not a u32"]
    }

    fn gen_input_states() -> Vec<ExecutionState> {
        let n: u32 = rand::thread_rng().next_u32();

        let mut even_stack = get_init_tvm_stack();
        let even_value = n - (n & 1);
        push_hashable(&mut even_stack, &even_value);

        let mut odd_stack = get_init_tvm_stack();
        let odd_value = n | 1;
        push_hashable(&mut odd_stack, &odd_value);

        let mut bfe_stack = get_init_tvm_stack();
        let bfe_value = BFieldElement::new(BFieldElement::MAX);
        push_hashable(&mut bfe_stack, &bfe_value);

        let mut above_max_even_stack = get_init_tvm_stack();
        let above_max_even_value = BFieldElement::new(2u64.pow(32));
        push_hashable(&mut above_max_even_stack, &above_max_even_value);

        let mut above_max_odd_stack = get_init_tvm_stack();
        let above_max_odd_value = BFieldElement::new(2u64.pow(32) + 1);
        push_hashable(&mut above_max_odd_stack, &above_max_odd_value);

        vec![
            ExecutionState::with_stack(even_stack),
            ExecutionState::with_stack(odd_stack),
            ExecutionState::with_stack(bfe_stack),
            ExecutionState::with_stack(above_max_even_stack),
            ExecutionState::with_stack(above_max_odd_stack),
        ]
    }
}

impl Snippet for U32IsOdd {
    fn stack_diff() -> isize {
        // pops a u32 from the stack and pushes a bool
        0
    }

    fn entrypoint() -> &'static str {
        "is_odd"
    }

    fn function_body(_library: &mut Library) -> String {
        let entrypoint = Self::entrypoint();
        format!(
            "
                // BEFORE: _ value
                // AFTER: _ (value % 2)
                {entrypoint}:
                    lsb
                    swap1
                    pop
                    return
                "
        )
    }

    fn rust_shadowing(
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        _memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        let value: u64 = stack.pop().unwrap().value();
        stack.push(BFieldElement::new(value % 2));
    }
}

#[cfg(test)]
mod u32_is_odd_tests {
    use rand::{thread_rng, RngCore};

    use crate::get_init_tvm_stack;
    use crate::snippet_bencher::bench_and_write;
    use crate::test_helpers::{rust_tasm_equivalence_prop, rust_tasm_equivalence_prop_new};

    use super::*;

    #[test]
    fn is_odd_u32_test() {
        rust_tasm_equivalence_prop_new::<U32IsOdd>();
    }

    #[test]
    fn is_odd_u32_benchmark() {
        bench_and_write::<U32IsOdd>();
    }

    #[test]
    fn u32_is_odd_test() {
        prop_is_odd(0);
        prop_is_odd(1);
        prop_is_odd(2);
        prop_is_odd(3);
        prop_is_odd(4);
        prop_is_odd(5);
        prop_is_odd(6);
        prop_is_odd(7);
        prop_is_odd(8);
        prop_is_odd(u32::MAX);

        let mut rng = thread_rng();
        for _ in 0..100 {
            let value = rng.next_u32();
            prop_is_odd(value);
        }
    }

    fn prop_is_odd(value: u32) {
        let mut init_stack = get_init_tvm_stack();
        init_stack.push(BFieldElement::new(value as u64));
        let mut expected_stack = get_init_tvm_stack();
        expected_stack.push(BFieldElement::new((value % 2) as u64));

        let _execution_result = rust_tasm_equivalence_prop::<U32IsOdd>(
            &init_stack,
            &[],
            &[],
            &mut HashMap::default(),
            0,
            Some(&expected_stack),
        );
    }
}
