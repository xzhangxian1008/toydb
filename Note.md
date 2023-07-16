# Task

- investigate the select procedure

# Select Procedure

- raft will not apply logs for `select`, but will drive state machine

# Insert Procedure

# Transaction

We have three types of transaction:
- raft.rs:Transaction: raft client directly interacts with this Transaction
- kv.rs:Transaction: it actually calls the mvcc.rs:Transaction in the implementation
- mvcc.rs:Transaction: raft interacts with this Transaction. Its interface will be called in apply operation
