use rome_diagnostics::v2::Diagnostic;

#[derive(Debug, Diagnostic)]
struct TestDiagnostic {
    #[location(unknown)]
    location: (),
}

fn main() {}
