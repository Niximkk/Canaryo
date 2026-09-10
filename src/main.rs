use std::env;

fn main() {
    let arguments: Vec<String> = env::args().collect();

    if arguments.len() == 1 {
        println!("Uso: canaryo <arquivo.js>");
        return;
    }

    println!("Canaryo recebeu: {}", arguments[1]);
}