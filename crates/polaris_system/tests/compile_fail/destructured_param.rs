use polaris_system::system;

#[system]
async fn destructured_system((a, b): (i32, i32)) -> i32 {
    a + b
}

fn main() {}
