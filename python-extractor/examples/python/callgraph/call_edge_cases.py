from typing import Any


# Edge case 1: Call with starred arguments — *args and **kwargs are captured
# as positional arguments in the extracted CallStatement
def delegate(target, *args, **kwargs):
    return target(*args, **kwargs)


# Edge case 2: Call inside a list comprehension — the function call foo(x)
# inside [foo(x) for x in items] should still be captured
def transform_all(items):
    return [str(x) for x in items]


# Edge case 3: isinstance() built-in call — captured as a regular CallStatement
# with function_name "isinstance" and two arguments
def handle_input(data: Any) -> str:
    if isinstance(data, str):
        return data
    return str(data)
