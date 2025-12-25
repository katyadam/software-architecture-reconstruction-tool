pub const ENTITIES_QUERY: &str = r#"
(package_declaration (_) @package)
(class_declaration
	name: 
    	(identifier) @entity.name
    superclass: (_ (type_identifier) @entity.superclass)?
	body: 
    	(class_body 
    		(field_declaration
            	type: (_)
                declarator: 
                [
                	(_                 	
						name: (_)
                		value: (_)
                    ) 
                    (_
                    	name: (_)
                    )
                ]
            ) 
        ) @entity.body
)
(record_declaration
	name: (identifier) @entity.name
    parameters: (_ 
    	(formal_parameter
        	type: (_)
            name: (_)
        )
    ) @entity.recordparams
    body: (class_body 
    		(field_declaration
            	type: (_)
                declarator: 
                [
                	(_                 	
						name: (_)
                		value: (_)
                    ) 
                    (_
                    	name: (_)
                    )
                ]
            )
        ) @entity.body
) @record
"#;

pub const IMPORTS_QUERY: &str = r#"
(import_declaration
	(scoped_identifier) @import.scoped
    (asterisk)? @import.asterisk
)
"#;

pub const ENDPOINTS_QUERY: &str = r#"
(method_declaration
	(modifiers [(annotation) (marker_annotation)]) @modifiers
    type: (_) @return_type
    name: (_) @callable_name
    parameters: (_) @callable_params
) @callable

(class_declaration
	(modifiers
		(annotation) @class_annotation
	)
)
"#;

pub const RESTCALLS_QUERY: &str = r#"
(method_invocation
	object: (_)? @invoked_on
    name: (_) @invoked_callable_name
    arguments: (_) @arguments
) @invoke
"#;

pub const CALLABLES_QUERY: &str = r#"
[
    (method_declaration
    type: (_)? @callable_type
    name: (_) @callable_name
    parameters: (_) @callable_params
    ) @type_method

    (constructor_declaration
    name: (_) @callable_name
    parameters: (_) @callable_params
    ) @type_constructor
] @callable
"#;

pub const CALL_STATEMENTS_QUERY: &str = r#"
[
    (object_creation_expression
        type: (_) @invoked_on
        arguments: (_) @args
    )
    (explicit_constructor_invocation
        constructor: (_) @explicit_constructor_call
        arguments: (_) @args
    )
    (method_invocation
        object: (_)? @invoked_on
        name: (_) @name
        arguments: (_) @args
    )
] @call
"#;
