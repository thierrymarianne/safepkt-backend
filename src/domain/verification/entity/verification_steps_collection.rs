use std::collections::HashMap;

pub type StepProvider = for<'a> fn(&'a str, &'a str) -> String;

pub struct Step {
    name: String,
    step_provider: StepProvider,
}

impl Step {
    pub fn new(name: String, step_provider: StepProvider) -> Step {
        Step {
            name,
            step_provider,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn step_provider(&self) -> &StepProvider {
        &self.step_provider
    }
}

pub struct VerificationStepsCollection<'a> {
    steps: HashMap<&'a str, Step>,
}

impl VerificationStepsCollection<'_> {
    pub fn new(steps: HashMap<&str, Step>) -> VerificationStepsCollection {
        VerificationStepsCollection { steps }
    }

    pub fn step(&self, name: &String) -> &Step {
        &self.steps.get(name.as_str()).unwrap()
    }
}
