import threading

results = []
lock = threading.Lock()

def producer(items):
    for item in items:
        with lock:
            results.append(item)

# Run two producers sequentially (real threading comes in Phase 7)
producer([1, 2, 3])
producer([4, 5, 6])

results.sort()
print(results)  # [1, 2, 3, 4, 5, 6]

# Basic lock usage
counter = [0]

def increment(n):
    for _ in range(n):
        with lock:
            counter[0] += 1

increment(100)
increment(100)
print(counter[0])  # 200
