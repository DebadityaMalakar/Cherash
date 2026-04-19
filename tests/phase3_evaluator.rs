use cherash::run_source;

fn run(src: &str) {
    run_source(src, false).unwrap_or_else(|e| panic!("runtime error: {}", e));
}

fn run_strict(src: &str) {
    run_source(src, true).unwrap_or_else(|e| panic!("runtime error: {}", e));
}

fn run_expect_err(src: &str, expected_type: &str) {
    match run_source(src, true) {
        Err(cherash::CherashError::TypeCheck(_)) => {} // type error is fine for type check cases
        Err(cherash::CherashError::Runtime(e)) => {
            assert_eq!(e.type_name, expected_type, "wrong error type: got {:?}", e);
        }
        Ok(()) => panic!("expected error '{}' but program succeeded", expected_type),
        Err(e) => panic!("unexpected error: {}", e),
    }
}

#[test]
fn test_fibonacci() {
    run(r#"
def fib(n):
    if n <= 1:
        return n
    return fib(n-1) + fib(n-2)

result = fib(10)
assert result == 55, f"expected 55, got {result}"
"#);
}

#[test]
fn test_closure_counter() {
    run(r#"
def counter():
    n = 0
    def inc():
        nonlocal n
        n += 1
        return n
    return inc

c = counter()
a = c()
b = c()
d = c()
assert a == 1
assert b == 2
assert d == 3
"#);
}

#[test]
fn test_fizzbuzz() {
    run(r#"
for i in range(1, 16):
    if i % 15 == 0:
        pass
    elif i % 3 == 0:
        pass
    elif i % 5 == 0:
        pass
    else:
        pass
"#);
}

#[test]
fn test_arithmetic_wrapping() {
    run(r#"
x = 9223372036854775807
y = x + 1
assert y == -9223372036854775808
"#);
}

#[test]
fn test_list_operations() {
    run(r#"
lst = [1, 2, 3]
lst2 = lst + [4, 5]
assert len(lst2) == 5
assert lst2[0] == 1
assert lst2[-1] == 5
"#);
}

#[test]
fn test_dict_operations() {
    run(r#"
d = {'a': 1, 'b': 2}
assert d['a'] == 1
d['c'] = 3
assert len(d) == 3
"#);
}

#[test]
fn test_while_break() {
    run(r#"
i = 0
while True:
    i += 1
    if i >= 5:
        break
assert i == 5
"#);
}

#[test]
fn test_for_else() {
    run(r#"
found = False
for x in [1, 2, 3]:
    if x == 2:
        found = True
        break
else:
    found = False
assert found == True
"#);
}

#[test]
fn test_string_ops() {
    run(r#"
s = "hello" + " " + "world"
assert s == "hello world"
assert len(s) == 11
assert s[0] == "h"
assert s[-1] == "d"
"#);
}

#[test]
fn test_nested_functions() {
    run(r#"
def make_adder(n):
    def adder(x):
        return x + n
    return adder

add5 = make_adder(5)
assert add5(3) == 8
assert add5(10) == 15
"#);
}

#[test]
fn test_list_comprehension() {
    run(r#"
squares = [x * x for x in range(5)]
assert len(squares) == 5
assert squares[0] == 0
assert squares[4] == 16
"#);
}

#[test]
fn test_try_except() {
    run(r#"
try:
    x = 1 // 0
except ZeroDivisionError:
    x = -1
assert x == -1
"#);
}

#[test]
fn test_class_basic() {
    run(r#"
class Dog:
    def __init__(self, name):
        self.name = name
    def bark(self):
        return "Woof! I am " + self.name

d = Dog("Rex")
result = d.bark()
assert result == "Woof! I am Rex"
"#);
}

#[test]
fn test_comparison_chaining() {
    run(r#"
assert 1 < 2 < 3
assert not (1 < 2 > 3)
assert 1 <= 1 <= 2
"#);
}

#[test]
fn test_type_error_strict() {
    run_expect_err(r#"
x: int = "hello"
"#, "TypeError");
}

#[test]
fn test_zero_division() {
    run_expect_err(r#"
x = 5 // 0
"#, "ZeroDivisionError");
}
