pub const ENTITIES_QUERY: &str = r#"
(class_declaration
	name: 
    	(identifier) @entity.name
    superclass: (_ (type_identifier) @entity.superclass)
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
