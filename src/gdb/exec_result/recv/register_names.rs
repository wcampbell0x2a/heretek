use crate::mi::parse_register_names_values;

/// `MIResponse::ExecResult`, key: "register-names"
pub fn recv_exec_result_register_names(register_name: &String, register_names: &mut Vec<String>) {
    let register_names_new = parse_register_names_values(register_name);
    *register_names = register_names_new;
}
