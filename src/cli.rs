// Este enum é o resultado da interpretação dos textos recebidos no terminal.
// Cada variante guarda apenas os dados necessários para executar aquele comando.
#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeMode {
    Native,
    Node,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Help,
    Version,
    Check {
        path: String,
    },
    Run {
        path: String,
        arguments: Vec<String>,
        runtime: RuntimeMode,
    },
}

pub fn parse(arguments: &[String]) -> Result<CliCommand, String> {
    if arguments.is_empty() {
        return Err("informe um comando ou arquivo JavaScript".into());
    }

    match arguments[0].as_str() {
        "--help" | "-h" => Ok(CliCommand::Help),
        "--version" | "-V" => Ok(CliCommand::Version),
        "check" => arguments
            .get(1)
            .map(|path| CliCommand::Check {
                path: path.to_owned(),
            })
            .ok_or_else(|| "informe o arquivo que deve ser analisado".into()),
        "run" => parse_run(&arguments[1..]),
        // Um caminho sem `run` é aceito como atalho para melhorar a ergonomia.
        path => Ok(CliCommand::Run {
            path: path.to_owned(),
            arguments: arguments[1..].to_vec(),
            runtime: RuntimeMode::Native,
        }),
    }
}

fn parse_run(arguments: &[String]) -> Result<CliCommand, String> {
    let (runtime, path_index) = if arguments.first().map(String::as_str) == Some("--node") {
        (RuntimeMode::Node, 1)
    } else {
        (RuntimeMode::Native, 0)
    };

    let path = arguments
        .get(path_index)
        .ok_or_else(|| "informe o arquivo que deve ser executado".to_string())?;

    Ok(CliCommand::Run {
        path: path.to_owned(),
        arguments: arguments[path_index + 1..].to_vec(),
        runtime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Este auxiliar deixa cada teste próximo da forma digitada no terminal.
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_the_check_command() {
        let command = parse(&args(&["check", "server.js"]));

        assert_eq!(
            command,
            Ok(CliCommand::Check {
                path: "server.js".into()
            })
        );
    }

    #[test]
    fn parses_the_run_shorthand_and_its_arguments() {
        let command = parse(&args(&["server.js", "--port", "3000"]));

        assert_eq!(
            command,
            Ok(CliCommand::Run {
                path: "server.js".into(),
                arguments: args(&["--port", "3000"]),
                runtime: RuntimeMode::Native,
            })
        );
    }

    #[test]
    fn parses_the_explicit_node_fallback() {
        let command = parse(&args(&["run", "--node", "server.js"]));

        assert_eq!(
            command,
            Ok(CliCommand::Run {
                path: "server.js".into(),
                arguments: Vec::new(),
                runtime: RuntimeMode::Node,
            })
        );
    }

    #[test]
    fn rejects_check_without_a_path() {
        assert_eq!(
            parse(&args(&["check"])),
            Err("informe o arquivo que deve ser analisado".into())
        );
    }
}
