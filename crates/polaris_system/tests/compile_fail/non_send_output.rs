use polaris_system::system;
use std::rc::Rc;

#[system]
async fn non_send_system() -> Rc<String> {
    Rc::new("hello".to_string())
}

fn main() {}
