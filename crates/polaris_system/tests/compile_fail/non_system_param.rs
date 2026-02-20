use polaris_system::system;

/// A parameter type that does not implement `SystemParam` should be rejected.
#[system]
async fn bad_param(x: String) -> i32 {
    x.len() as i32
}

fn main() {}
