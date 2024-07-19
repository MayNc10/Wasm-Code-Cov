(module
	(func $fd_write (import "wasi_unstable" "fd_write") (param i32 i32 i32 i32) (result i32))
	(func $proc_exit (import "wasi_unstable" "proc_exit") (param i32))

	(memory (export "memory") 1)
	(data (offset (i32.const 20)) "TEST\n")

	(func (export "_start")
		(i32.store (i32.const 0) (i32.const 20)) ;; iov.iov_base
		(i32.store (i32.const 4) (i32.const 5)) ;; iov.iov_len

		(call $fd_write
			(i32.const 1) ;; fd, 1 = stdout
			(i32.const 0) ;; *iovs
			(i32.const 1) ;; iovs_len
			(i32.const 4) ;; nwritten
		)

		(call $proc_exit)
	)
)
