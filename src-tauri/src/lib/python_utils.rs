use log::{info, trace};
#[cfg(feature = "userustpython")]
use rustpython_vm as vm;
#[cfg(feature = "userustpython")]
use rustpython_vm::function::PosArgs;
#[cfg(feature = "userustpython")]
use std::process::ExitCode;
#[cfg(feature = "userustpython")]
use vm::{builtins::PyStrRef, Interpreter};

use crate::{command_executor, replace_unescaped_spaces_posix, replace_unescaped_spaces_win};

/// Runs a Python script from a specified file with optional arguments and environment variables.
/// todo: check documentation
/// # Parameters
///
/// * `path` - A reference to a string representing the path to the Python script file.
/// * `args` - An optional reference to a string representing the arguments to be passed to the Python script.
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
/// * `envs` - An optional reference to a vector of tuples representing environment variables to be set for the Python script.
///
/// # Returns
///
/// * `Result<String, String>` - On success, returns a `Result` containing the standard output of the Python script as a string.
///   On error, returns a `Result` containing the standard error of the Python script as a string.
pub fn run_python_script_from_file(
    path: &str,
    args: Option<&str>,
    python: Option<&str>,
    envs: Option<&Vec<(String, String)>>,
) -> Result<String, String> {
    let callable = if let Some(args) = args {
        format!("{} {} {}", python.unwrap_or("python3"), path, args)
    } else {
        format!("{} {}", python.unwrap_or("python3"), path)
    };
    let executor = command_executor::get_executor();

    let output = match envs {
        Some(envs) => {
            let envs_str = envs
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<Vec<(&str, &str)>>();

            match std::env::consts::OS {
                "windows" => executor.execute_with_env(
                    "powershell",
                    &vec![
                        "-Command",
                        python.unwrap_or("python3.exe"),
                        path,
                        args.unwrap_or(""),
                    ],
                    envs_str,
                ),
                _ => executor.execute_with_env("bash", &vec!["-c", &callable], envs_str),
            }
        }
        None => match std::env::consts::OS {
            "windows" => executor.execute(
                "powershell",
                &vec![
                    "-Command",
                    python.unwrap_or("python3.exe"),
                    path,
                    args.unwrap_or(""),
                ],
            ),
            _ => executor.execute("bash", &vec!["-c", &callable]),
        },
    };

    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(std::str::from_utf8(&out.stdout).unwrap().to_string())
            } else {
                Err(std::str::from_utf8(&out.stderr).unwrap().to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Runs the IDF tools Python installation script.
///
/// This function prepares the environment to run a Python installation script for
/// IDF tools by ensuring that the path is properly escaped based on the operating
/// system. It then executes the installation script followed by the Python environment
/// setup script.
///
/// # Parameters
///
/// - `idf_tools_path`: A string slice that represents the path to the IDF tools.
/// - `environment_variables`: A vector of tuples containing environment variable names
///   and their corresponding values, which will be passed to the installation scripts.
///
/// # Returns
///
/// This function returns a `Result<String, String>`. On success, it returns an `Ok`
/// containing the output of the Python environment setup script. On failure, it returns
/// an `Err` containing an error message.
///
/// # Example
///
/// ```rust
/// use idf_im_lib::python_utils::run_idf_tools_py;
/// let path = "path/to/idf_tools";
/// let env_vars = vec![("VAR_NAME".to_string(), "value".to_string())];
/// match run_idf_tools_py(path, &env_vars) {
///     Ok(output) => println!("Success: {}", output),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```

pub fn run_idf_tools_py(
    // todo: rewrite functionality to rust
    idf_tools_path: &str,
    environment_variables_install: &Vec<(String, String)>,
    environment_variables_python: &Vec<(String, String)>,
) -> Result<String, String> {
    run_idf_tools_py_with_features(
        idf_tools_path,
        environment_variables_install,
        environment_variables_python,
        &vec![],
    )
}

pub fn run_idf_tools_py_with_features(
    idf_tools_path: &str,
    environment_variables_install: &Vec<(String, String)>,
    environment_variables_python: &Vec<(String, String)>,
    features: &Vec<String>,
) -> Result<String, String> {
    let escaped_path = if std::env::consts::OS == "windows" {
        replace_unescaped_spaces_win(&idf_tools_path)
    } else {
        replace_unescaped_spaces_posix(&idf_tools_path)
    };
    match run_install_script(&escaped_path, environment_variables_install) {
        Ok(_) => {
            info!("install success");
        }
        Err(e) => {
            info!("Instal failed {:?}", e);
        }
    }
    run_install_python_env_script_with_features(
        &escaped_path,
        environment_variables_python,
        features,
    )
}

fn run_install_script(
    idf_tools_path: &str,
    environment_variables: &Vec<(String, String)>,
) -> Result<String, String> {
    let output = run_python_script_from_file(
        idf_tools_path,
        Some("install"),
        None,
        Some(environment_variables),
    );

    trace!("idf_tools.py install output:\n{:?}", output);

    output
}

fn run_install_python_env_script(
    idf_tools_path: &str,
    environment_variables: &Vec<(String, String)>,
) -> Result<String, String> {
    let output =
        run_install_python_env_script_with_features(idf_tools_path, environment_variables, &vec![]);

    trace!("idf_tools.py install-python-env output:\n{:?}", output);

    output
}

fn run_install_python_env_script_with_features(
    idf_tools_path: &str,
    environment_variables: &Vec<(String, String)>,
    features: &Vec<String>,
) -> Result<String, String> {
    let mut args = "install-python-env".to_string();
    if features.len() > 0 {
        args = format!("{} --features {}", args, features.join(","));
    }
    let output = run_python_script_from_file(
        idf_tools_path,
        Some(&args),
        None,
        Some(environment_variables),
    );

    trace!("idf_tools.py install-python-env output:\n{:?}", output);

    output
}

/// Executes a Python script using the provided Python interpreter and returns the script's output.
///
/// # Parameters
///
/// * `script` - A reference to a string representing the Python script to be executed.
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
///   If `None`, the function will default to using "python3".
///
/// # Returns
///
/// * `Result<String, String>` - On success, returns a `Result` containing the standard output of the Python script as a string.
///   On error, returns a `Result` containing the standard error of the Python script as a string.
pub fn run_python_script(script: &str, python: Option<&str>) -> Result<String, String> {
    let output = command_executor::execute_command(python.unwrap_or("python3"), &["-c", &script]);
    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(std::str::from_utf8(&out.stdout).unwrap().to_string())
            } else {
                Err(std::str::from_utf8(&out.stderr).unwrap().to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Retrieves the platform definition by the Python interpreter.
///
/// This function executes a Python script that uses the `platform` module to determine the system and machine
/// details of the Python interpreter. The platform definition is formatted as "system-machine".
///
/// # Parameters
///
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
///   If `None`, the function will default to using "python3".
///
/// # Returns
///
/// * `String` - The platform definition of the Python interpreter. If the Python script execution fails,
///   the function returns the error message as a string.
pub fn get_python_platform_definition(python: Option<&str>) -> String {
    match run_python_script(
        "import platform; print(f'{platform.system()}-{platform.machine()}')",
        python,
    ) {
        Ok(out) => out,
        Err(e) => e,
    }
}

/// Performs a series of sanity checks for the Python interpreter.
///
/// This function executes various Python scripts and checks for the availability of essential Python modules,
/// such as pip, venv, and the standard library. It also verifies the functionality of the ctypes module.
///
/// # Parameters
///
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
///   If `None`, the function will default to using "python3".
///
/// # Returns
///
/// * `Vec<Result<String, String>>` - A vector of results. Each result represents the output or error message
///   of a specific Python script execution. If the script execution is successful, the result will be `Ok`
///   containing the standard output as a string. If the script execution fails, the result will be `Err`
///   containing the standard error as a string.
pub fn python_sanity_check(python: Option<&str>) -> Vec<Result<String, String>> {
    let mut outputs = Vec::new();
    // check pip
    let output =
        command_executor::execute_command(python.unwrap_or("python3"), &["-m", "pip", "--version"]);
    match output {
        Ok(out) => {
            if out.status.success() {
                outputs.push(Ok(std::str::from_utf8(&out.stdout).unwrap().to_string()));
            } else {
                outputs.push(Err(std::str::from_utf8(&out.stderr).unwrap().to_string()));
            }
        }
        Err(e) => outputs.push(Err(e.to_string())),
    }
    // check venv
    let output_2 =
        command_executor::execute_command(python.unwrap_or("python3"), &["-m", "venv", "-h"]);
    match output_2 {
        Ok(out) => {
            if out.status.success() {
                outputs.push(Ok(std::str::from_utf8(&out.stdout).unwrap().to_string()));
            } else {
                outputs.push(Err(std::str::from_utf8(&out.stderr).unwrap().to_string()));
            }
        }
        Err(e) => outputs.push(Err(e.to_string())),
    }
    // check standard library
    let script = include_str!("../../python_scripts/sanity_check/import_standard_library.py");
    outputs.push(run_python_script(script, python));
    // check ctypes
    let script = include_str!("../../python_scripts/sanity_check/ctypes_check.py");
    outputs.push(run_python_script(script, python));
    // check https
    let script = include_str!("../../python_scripts/sanity_check/import_standard_library.py");
    outputs.push(run_python_script(script, python));
    outputs
}

#[cfg(feature = "userustpython")]
pub fn run_python_script_with_rustpython(script: &str) -> String {
    vm::Interpreter::without_stdlib(Default::default()).enter(|vm| {
        let scope = vm.new_scope_with_builtins();
        let code_opbject = vm
            .compile(script, vm::compiler::Mode::Exec, "<embeded>".to_owned())
            .map_err(|err| format!("error: {:?}", err))
            .unwrap();
        let output = vm.run_code_obj(code_opbject, scope).unwrap();
        format!("output: {:?}", output)
        // Ok(output)
    });
    "".to_string()
}

#[cfg(feature = "userustpython")]
pub fn py_main_idf(interp: &Interpreter) -> vm::PyResult<PyStrRef> {
    interp.enter(|vm| {
        // Add local library path
        vm.insert_sys_path(vm.new_pyobj("examples"))
            .expect("add examples to sys.path failed, why?");

        // select the idf_tools module
        let module = vm.import("idf_tools", 0)?;
        // running straight the action_install
        let name_func = module.get_attr("action_install", vm)?;
        // we will get the params from the user in the future
        let quiet = vm.ctx.false_value.clone();
        let non_interactive = vm.ctx.new_bool(false);
        let tools_json = vm.ctx.new_str("./examples/tools.json");
        let idf_path = vm.ctx.none();
        let tools = vm.ctx.new_list(vec![vm.ctx.new_str("all").into()]);
        let targets = vm.ctx.new_str("all");

        let pos_args: PosArgs = PosArgs::new(vec![
            quiet.into(),
            non_interactive.into(),
            tools_json.into(),
            idf_path,
            tools.into(),
            targets.into(),
        ]);

        let result = name_func.call(pos_args, vm)?;
        let result_str = result.str(vm)?;
        let result_pystrref: PyStrRef = result_str;
        // let result: PyStrRef = result.get_attr("name", vm)?.try_into_value(vm)?;
        vm::PyResult::Ok(result_pystrref)
    })
}

#[cfg(feature = "userustpython")]
// in the future we will accept params what to actually install ;-)
pub fn run_idf_tools() -> ExitCode {
    let mut settings = vm::Settings::default();
    settings.path_list.push("Lib".to_owned()); // addng folder lib in current directory
    if let Ok(path) = env::var("RUSTPYTHONPATH") {
        settings
            .path_list
            .extend(path.split(':').map(|s| s.to_owned()));
    }
    let interp = vm::Interpreter::with_init(settings, |vm| {
        vm.add_native_modules(rustpython_stdlib::get_module_inits());
    });

    let result = py_main_idf(&interp);
    let result = result.map(|result| {
        println!("name: {result}");
    });
    ExitCode::from(interp.run(|_vm| result))
}
