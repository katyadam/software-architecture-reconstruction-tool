pub const RESTCALLS_QUERY: &str = r#"
;; ────────────────────────────────────────────────────────────────────────────
;; PATTERN 1 — no with_statement
;; ────────────────────────────────────────────────────────────────────────────
(function_definition 
  name: (identifier) @function.name
  parameters: (parameters) @function.parameters
  body: (block
    (expression_statement
      [
        ;; ─── await without assignment ────────────────────────────────
        (await
          (call
            function: (attribute
              attribute: (identifier) @http.method
              (#any-of? @http.method "get" "post" "put" "delete")
            )
            arguments: (argument_list (string) @uri) @call.args
          )
        )

        ;; ─── non-await without assignment ────────────────────────────
        (call
          function: (attribute
            attribute: (identifier) @http.method
            (#any-of? @http.method "get" "post" "put" "delete")
          )
          arguments: (argument_list (string) @uri) @call.args
        )

        ;; ─── assignments ─────────────────────────────────────────────
        (assignment
          right: [
            ;; await assignment
            (await
              (call
                function: (attribute
                  attribute: (identifier) @http.method
                  (#any-of? @http.method "get" "post" "put" "delete")
                )
                arguments: (argument_list (string) @uri) @call.args
              )
            )
            ;; non-await assignment
            (call
              function: (attribute
                attribute: (identifier) @http.method
                (#any-of? @http.method "get" "post" "put" "delete")
              )
              arguments: (argument_list (string) @uri) @call.args
            )
          ]
        )
      ]
    )
  )
) @function

;; ────────────────────────────────────────────────────────────────────────────
;; PATTERN 2 — with_statement → block → expression_statement
;; ────────────────────────────────────────────────────────────────────────────
(function_definition 
  name: (identifier) @function.name
  parameters: (parameters) @function.parameters
  body: (block
    (with_statement
      (block
        (expression_statement
          [
            ;; ─── await without assignment ────────────────────────────────
            (await
              (call
                function: (attribute
                  attribute: (identifier) @http.method
                  (#any-of? @http.method "get" "post" "put" "delete")
                )
                arguments: (argument_list (string) @uri) @call.args
              )
            )

            ;; ─── non-await without assignment ────────────────────────────
            (call
              function: (attribute
                attribute: (identifier) @http.method
                (#any-of? @http.method "get" "post" "put" "delete")
              )
              arguments: (argument_list (string) @uri) @call.args
            )

            ;; ─── assignments ─────────────────────────────────────────────
            (assignment
              right: [
                ;; await assignment
                (await
                  (call
                    function: (attribute
                      attribute: (identifier) @http.method
                      (#any-of? @http.method "get" "post" "put" "delete")
                    )
                    arguments: (argument_list (string) @uri) @call.args
                  )
                )
                ;; non-await assignment
                (call
                  function: (attribute
                    attribute: (identifier) @http.method
                    (#any-of? @http.method "get" "post" "put" "delete")
                  )
                  arguments: (argument_list (string) @uri) @call.args
                )
              ]
            )
          ]
        )
      )
    )
  )
) @function
"#;

pub const ENDPOINTS_QUERY: &str = r#"
(decorated_definition
    (decorator
        (call (attribute attribute: (identifier) @http.method)
        (argument_list (string) @http.uri)
    ) @finder (#match? @finder "app."))
    (function_definition 
        name: (identifier) @function.name
        parameters: (parameters) @function.params) @function
)
"#;

pub const ASSINGMENTS_QUERY: &str = r#"
(assignment left: (_) @variable right: (_) @value)
(assignment left: (_) @variable type: (_) @type right: (_) @value)
(function_definition
    name: (identifier) @function.name
    parameters: (parameters) @function.params
	return_type: (_)? @function.return_type
)
"#;

pub const ENTITIES_QUERY: &str = r#"
(
  (class_definition
    (identifier) @class.name
    (argument_list) @class.superclasses
    (block
      (expression_statement)* @class.fields))
)

(
  (class_definition
    (identifier) @class.name
    (argument_list) @class.superclasses
    (block
      (function_definition
        (identifier) @constructor.name (#match? @constructor.name "__init__")
		(parameters) @class.fields
	   )
	)
  )
)
"#;

pub const CALLABLES_QUERY: &str = r#"
[
;; Case 1a: Undecorated function directly inside a class body
(
  (class_definition
    name: (identifier) @class.name
    body: (block
      (function_definition
        name: (identifier) @function.name
        parameters: (parameters) @function.params
        (#optional? return_type)
        return_type: (type)? @function.return_type
        body: (block) @function.body
      ) @function
    )
  )
)

;; Case 1b: Decorated function inside a class body (@staticmethod, @classmethod, @property, etc.)
(
  (class_definition
    name: (identifier) @class.name
    body: (block
      (decorated_definition
        definition: (function_definition
          name: (identifier) @function.name
          parameters: (parameters) @function.params
          (#optional? return_type)
          return_type: (type)? @function.return_type
          body: (block) @function.body
        ) @function
      )
    )
  )
)

;; Case 2: Top-level function that does NOT have a class_definition ancestor
(
  (function_definition
    name: (identifier) @function.name
    parameters: (parameters) @function.params
    (#optional? return_type)
    return_type: (type)? @function.return_type
    body: (block) @function.body
  ) @function
)
  ]
"#;

pub const CALL_QUERY: &str = r#"
(call 
	[
		(attribute) @call.ident
		(identifier) @call.ident
	]
    (argument_list) @call.args
) @call
"#;

pub const IMPORTS_QUERY: &str = r#"
[
  (import_statement 
    [
        name: (dotted_name) @import.module
        name: (aliased_import) @aliased_import.module
    ]
    
  )
  (import_from_statement
    module_name: [(dotted_name) (relative_import)] @from_import.module 
    [
      name: (dotted_name) @from_import.names
          name: (aliased_import) @from_aliased_import.names

      ]
  )
]
"#;
