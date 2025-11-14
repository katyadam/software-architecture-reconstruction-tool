use models::{Argument, CallStatement, Parameter};

use crate::imcg::model::ServiceCallable;

fn match_arg_and_param_datatypes(arg_datatype: &str, param_datatype: &Option<String>) -> bool {
    match param_datatype {
        Some(u_param_datatype) => arg_datatype == u_param_datatype,
        None => "any" == arg_datatype,
    }
}

pub(crate) fn get_score(callable: &ServiceCallable, call_statement: &CallStatement) -> i32 {
    if let Some(function_name) = callable.callable.name.split("(").next() {
        if function_name != call_statement.function_name {
            return 0;
        }
    }

    let matches = call_statement
        .arguments
        .iter()
        .zip(&callable.callable.parameters)
        .filter(|(arg, param)| match_arg_and_param_datatypes(&arg.datatype, &param.datatype))
        .count();

    1 + matches as i32
}
