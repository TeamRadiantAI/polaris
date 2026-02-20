use polaris_system::system;

struct Agent;

impl Agent {
    #[system]
    async fn system_with_self(&self) -> i32 {
        42
    }
}

fn main() {}
