- added import to end of imports section
bumped every canon resource.drop with an idx >= 18
bumped every canon lower func
bump type indices in things like `(type (;23;) (func (result 22)))` and `func (;15;) (type 23)`
bump func indices in component func usages in things like `(instance (;10;) (instantiate 0
      (with "import-func-run" (func 15))
    )
  )`
Lower the imported function to a core function, and make an instance exporting that function
`(core func $inc-counter (canon lower (func $inc-counter)))
  (core instance
   (export "inc-counter" (func $inc-counter))
  )`
bump all instance idxs after making that instance (like in `alias core export idx ` statements)
bump indexes in `(export "func" (func idx))` statements
bump function indexes in  `(realloc idx)` statements
bump function indexes in `(canon lift (core func idx))` statements
