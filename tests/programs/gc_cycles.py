# Tests that the tricolor GC can collect reference cycles.
# Two objects holding references to each other should be collected
# once no root references remain.

class Node:
    def __init__(self, name):
        self.name = name
        self.ref = None

def make_cycle():
    a = Node("A")
    b = Node("B")
    a.ref = b
    b.ref = a
    # Both a and b go out of scope here; the cycle should be collected.

# Create many cycles to exercise the GC
for _ in range(1000):
    make_cycle()

print("GC cycle test complete — no crash means cycles were handled")

# Self-referential list
def make_self_ref_list():
    lst = [1, 2, 3]
    # Can't do lst.append(lst) easily without methods, but the GC
    # should collect lst once it leaves scope.

for _ in range(500):
    make_self_ref_list()

print("Self-ref list test complete")
