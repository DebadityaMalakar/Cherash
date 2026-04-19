def divide(a, b):
    if b == 0:
        raise ZeroDivisionError("division by zero")
    return a // b

# Basic try/except
try:
    result = divide(10, 0)
except ZeroDivisionError as e:
    print("caught:", e)

# Try/except/else/finally
try:
    x = divide(10, 2)
except ZeroDivisionError:
    print("error")
else:
    print("result:", x)   # result: 5
finally:
    print("done")          # done

# Nested exceptions
try:
    try:
        raise ValueError("inner")
    except ValueError as e:
        print("inner caught:", e)
        raise RuntimeError("outer")
except RuntimeError as e:
    print("outer caught:", e)

# Multiple except clauses
def risky(n):
    if n == 0:
        raise ZeroDivisionError("zero")
    if n < 0:
        raise ValueError("negative")
    return n * 2

for val in [1, 0, -1]:
    try:
        print(risky(val))
    except ZeroDivisionError:
        print("zero error")
    except ValueError:
        print("value error")
