use cherash::run_source;

fn run(src: &str) -> String {
    // capture stdout by running and returning the printed value via eval
    // We use a simpler approach: run and check it doesn't panic/error
    match run_source(src, false) {
        Ok(_) => "ok".into(),
        Err(e) => format!("ERROR: {}", e),
    }
}

#[test]
fn test_lock_basic() {
    let result = run(r#"
import threading
lock = threading.Lock()
lock.acquire()
lock.release()
"#);
    assert_eq!(result, "ok");
}

#[test]
fn test_lock_context_manager() {
    let result = run(r#"
import threading
lock = threading.Lock()
x = 0
with lock:
    x = x + 1
"#);
    assert_eq!(result, "ok");
}

#[test]
fn test_thread_spawn_and_join() {
    // Spawn a real OS thread that modifies shared state protected by a lock
    let result = run(r#"
import threading

results = []
lock = threading.Lock()

def worker(n):
    with lock:
        results.append(n)

t1 = threading.Thread(worker, [1])
t2 = threading.Thread(worker, [2])
t3 = threading.Thread(worker, [3])

t1.start()
t2.start()
t3.start()

t1.join()
t2.join()
t3.join()
"#);
    assert_eq!(result, "ok");
}

#[test]
fn test_thread_counter() {
    // Multiple threads increment a shared counter via a lock
    let result = run(r#"
import threading

counter = [0]
lock = threading.Lock()

def increment():
    i = 0
    while i < 100:
        with lock:
            counter[0] = counter[0] + 1
        i = i + 1

t1 = threading.Thread(increment, [])
t2 = threading.Thread(increment, [])

t1.start()
t2.start()

t1.join()
t2.join()
"#);
    assert_eq!(result, "ok");
}

#[test]
fn test_multiple_locks() {
    let result = run(r#"
import threading

lock_a = threading.Lock()
lock_b = threading.Lock()

with lock_a:
    with lock_b:
        x = 42
"#);
    assert_eq!(result, "ok");
}
