use models::CallStatement;

use crate::imcg::model::ServiceCallable;

fn match_arg_and_param_datatypes(arg_datatype: &str, param_datatype: &Option<String>) -> bool {
    match param_datatype {
        Some(u_param_datatype) => arg_datatype == u_param_datatype,
        None => "any" == arg_datatype,
    }
}

pub(crate) fn get_score(callable: &ServiceCallable, call_stmt: &CallStatement) -> i32 {
    // 1. Function name must match
    let callable_name = callable.callable.name.split('(').next().unwrap_or_default();

    if callable_name != call_stmt.function_name {
        return 0;
    }

    // 2. Count datatype matches
    let match_count = call_stmt
        .arguments
        .iter()
        .zip(&callable.callable.parameters)
        .filter(|(arg, param)| match_arg_and_param_datatypes(&arg.datatype, &param.datatype))
        .count();

    // 3. Base score from argument count match
    let base_score = if call_stmt.arguments.len() == callable.callable.parameters.len() {
        2
    } else {
        1
    };

    base_score + match_count as i32
}
