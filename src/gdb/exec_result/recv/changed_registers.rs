use crate::mi::parse_register_names_values;

/// `MIResponse::ExecResult`, key: "changed-registers"
pub fn recv_exec_result_changed_registers(
    changed_registers: &String,
    register_changed: &mut Vec<u8>,
) {
    let changed_registers = parse_register_names_values(changed_registers);
    let result: Vec<u8> =
        changed_registers.iter().map(|s| s.parse::<u8>().expect("Invalid number")).collect();
    *register_changed = result;
}
