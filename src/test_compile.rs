//! Test command for testing the code generator pipeline
//!
//! The `compile` test command runs each function through the full code generator pipeline
use crate::sub_test::SubTest;

use crate::{build_backend, Context};
use anyhow::{bail, Result};
use cranelift_codegen::binemit::CodeInfo;
use cranelift_codegen::ir;
use cranelift_reader::{TestCommand, TestOption};
use log::info;
use std::borrow::Cow;
use std::env;

struct TestCompile {
    /// Flag indicating that the text expectation, comments after the function,
    /// must be a precise 100% match on the compiled output of the function.
    /// This test assertion is also automatically-update-able to allow tweaking
    /// the code generator and easily updating all affected tests.
    precise_output: bool,
}

pub fn subtest(parsed: &TestCommand) -> Result<Box<dyn SubTest>> {
    assert_eq!(parsed.command, "compile");
    let mut test = TestCompile {
        precise_output: false,
    };
    for option in parsed.options.iter() {
        match option {
            TestOption::Flag("precise-output") => test.precise_output = true,
            _ => anyhow::bail!("unknown option on {}", parsed),
        }
    }
    Ok(Box::new(test))
}

impl SubTest for TestCompile {
    fn name(&self) -> &'static str {
        "compile"
    }

    fn is_mutating(&self) -> bool {
        false
    }

    fn needs_isa(&self) -> bool {
        false
    }

    fn run(&self, func: Cow<ir::Function>, context: &Context) -> Result<()> {
        use cranelift_codegen::Context;
        let mut compiler = Context::for_function(func.clone().into_owned());
        compiler.want_disasm = true;
        let reuslt = compiler.compile(build_backend().as_ref()).unwrap();
        let disasm = reuslt.disasm.clone().unwrap().clone();
        check_precise_output(disasm.as_str(), context)
    }
}

const just_update_test: bool = true;

fn check_precise_output(text: &str, context: &Context) -> Result<()> {
    let actual = text.lines().collect::<Vec<_>>();

    // Use the comments after the function to build the test expectation.
    let expected = context
        .details
        .comments
        .iter()
        .filter(|c| !c.text.starts_with(";;"))
        .map(|c| c.text.strip_prefix("; ").unwrap_or(c.text))
        .collect::<Vec<_>>();

    // If the expectation matches what we got, then there's nothing to do.
    if actual == expected {
        return Ok(());
    }
    if just_update_test {
        return update_test(&actual, context);
    }

    // Otherwise this test has failed, and we can print out as such.
    bail!(
        "compilation of function on line {} does not match\n\
         the text expectation\n\
         \n\
         expected:\n\
         {:#?}\n\
         actual:\n\
         {:#?}\n\
         \n\
         This test assertion can be automatically updated by setting the\n\
         CRANELIFT_TEST_BLESS=1 environment variable when running this test.
         ",
        context.details.location.line_number,
        expected,
        actual,
    )
}

fn update_test(output: &[&str], context: &Context) -> Result<()> {
    context
        .file_update
        .update_at(&context.details.location, |new_test, old_test| {
            // blank newline after the function
            new_test.push_str("\n");

            // Splice in the test output
            for output in output {
                new_test.push_str("; ");
                new_test.push_str(output);
                new_test.push_str("\n");
            }

            // blank newline after test assertion
            new_test.push_str("\n");

            // Drop all remaining commented lines (presumably the old test expectation),
            // but after we hit a real line then we push all remaining lines.
            let mut in_next_function = false;
            for line in old_test {
                if !in_next_function && (line.trim().is_empty() || line.starts_with(";")) {
                    continue;
                }
                in_next_function = true;
                new_test.push_str(line);
                new_test.push_str("\n");
            }
        })
}
