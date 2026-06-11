use crate::Value;
use super::ExecuteResult;

pub fn print(args: &[Value]) -> ExecuteResult<Value> {
    for (i, arg) in args.iter().enumerate() {
        if i == 0 {
            print!("{arg}");
        } else {
            print!(" {arg}");
        }
    }
    println!("");
    Ok(Value::Nil)
}