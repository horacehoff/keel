use crate::Instr;
use crate::parse;

macro_rules! run_and_check_registers {
    ($contents:expr, $expected:expr) => {
        let filename = "test.kl";
        let (
            instructions,
            mut registers,
            mut arrays,
            instr_src,
            fn_registers,
            _,
            allocated_arg_count,
            allocated_call_depth,
            _,
        ) = parse($contents, filename, true);
        crate::vm::execute(
            &instructions,
            &mut registers,
            &mut arrays,
            &crate::errors::ErrorCtx {
                instr_src,
                sources: vec![(filename.into(), $contents.into())],
            },
            &fn_registers,
            &[],
            allocated_arg_count,
            allocated_call_depth,
        );
        assert!(instructions.iter().any(|x| {
            if let Instr::Print(tgt) = x {
                registers[(*tgt) as usize] == $expected
            } else {
                false
            }
        }));
    };
}

macro_rules! run {
    ($contents:expr) => {
        let filename = "test.kl";
        let (
            instructions,
            mut registers,
            mut arrays,
            instr_src,
            fn_registers,
            _,
            allocated_arg_count,
            allocated_call_depth,
            _,
        ) = parse($contents, filename, true);
        crate::vm::execute(
            &instructions,
            &mut registers,
            &mut arrays,
            &crate::errors::ErrorCtx {
                instr_src,
                sources: vec![(filename.into(), $contents.into())],
            },
            &fn_registers,
            &[],
            allocated_arg_count,
            allocated_call_depth,
        );
    };
}

#[test]
pub fn rec_fib_1() {
    run_and_check_registers!(
        "
        function fib(n) {
            if n <= 1 {return n;}
            else {return fib(n-1)+fib(n-2);}
        }

        function main() {
            let x = fib(1);
            print(x);
        }
        ",
        1.into()
    );
}

#[test]
pub fn rec_fib_25() {
    run_and_check_registers!(
        "
        function fib(n) {
            if n <= 1 {return n;}
            else {return fib(n-1)+fib(n-2);}
        }

        function main() {
            let x = fib(25);
            print(x);
        }
        ",
        75025.into()
    );
}

#[test]
pub fn fn_call_in_if_in_for() {
    run_and_check_registers!(
        "
        function is_digit(c) {
            return c == \"0\" || c == \"1\" || c == \"2\" || c == \"3\" || c == \"4\" || c == \"5\" || c == \"6\" || c == \"7\" || c == \"8\" || c == \"9\";
        }
        function main() {
            let count = 0;
            for x in \"3 + 4\" {
                if x != \" \" {
                    if is_digit(x) {
                        count += 1;
                    }
                }
            }
            print(count);
        }
        ",
        2.into()
    );
}

#[test]
pub fn while_and_condition() {
    run_and_check_registers!(
        "
        function main() {
        let count = 0;
        let limit = 1000000;
        let result = 1;
        while count < limit {
            result *= 2;
            if result > 1000000 {
                result %= 1000000;
            }
            count += 1;
        }
        print(result);
        }
        ",
        109376.into()
    );
}

#[test]
pub fn iter_fib_40() {
    run_and_check_registers!(
        "
        function main() {
        let n = 40;
        let a=0;
        let b=1;
        let c=0;
        let i=0;
        while i < (n-1) {
           c = a+b;
           a = b;
           b = c;
           i = i+1;
        }
        print(c);
        }
        ",
        102334155.into()
    );
}
#[test]
pub fn iter_fib_40_loop() {
    run_and_check_registers!(
        "
        function main() {
            let sum = 0;
            for _ in 0..200000 {
                let a = 0;
                let b = 1;
                let c = 0;
                for i in 0..39 {
                    c = a + b;
                    a = b;
                    b = c;
                }
                sum += (b % 10);
            }
            print(sum);
        }
        ",
        1000000.into()
    );
}

#[test]
pub fn string_split_len() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello world";
            let parts = s.split(" ");
            print(parts.len());
        }
        "#,
        2.into()
    );
}

#[test]
pub fn string_contains_replace() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello world";
            print(s.contains("world"));
        }
        "#,
        true.into()
    );
}

#[test]
pub fn for_loop_sum() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [1, 2, 3, 4, 5];
            let sum = 0;
            for x in arr {
                sum += x;
            }
            print(sum);
        }
        ",
        15.into()
    );
}

#[test]
pub fn array_sort() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [3, 1, 4, 1, 5, 9, 2, 6];
            arr.sort();
            print(arr[0]);
        }
        ",
        1.into()
    );
}

#[test]
pub fn array_push_len() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [1, 2, 3];
            arr.push(4);
            print(arr.len());
        }
        ",
        4.into()
    );
}

#[test]
pub fn array_partition() {
    run_and_check_registers!(
        "
        function main() {
            let x = [1,2,3,0,4,5,6];
            let p = x.partition(0);
            print(p[0][0]+p[1][2]);
        }
        ",
        7.into()
    );
}

#[test]
pub fn int_for_loop() {
    run_and_check_registers!(
        "
        function main() {
            let sum = 0;
            for i in 0..10 {
                sum += i;
            }
            print(sum);
        }
        ",
        45.into()
    );
}

#[test]
pub fn string_trim() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "  hello  ";
            let t = s.trim();
            print(t.len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn recursive_factorial() {
    run_and_check_registers!(
        "
        function fact(n) {
            if n <= 1 { return 1; }
            else { return n * fact(n - 1); }
        }
        function main() {
            print(fact(10));
        }
        ",
        3628800.into()
    );
}

#[test]
pub fn inline_condition_basic() {
    run_and_check_registers!(
        "
        function main() {
            let x = 10;
            let result = if x > 5 { 1 } else { 0 };
            print(result);
        }
        ",
        1.into()
    );
}

#[test]
pub fn inline_condition_else_branch() {
    run_and_check_registers!(
        "
        function main() {
            let x = 3;
            let result = if x > 5 { 1 } else { 0 };
            print(result);
        }
        ",
        0.into()
    );
}

#[test]
pub fn inline_condition_else_if() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            let result = if x > 10 { 2 } else if x > 3 { 1 } else { 0 };
            print(result);
        }
        ",
        1.into()
    );
}

#[test]
pub fn inline_condition_as_arg() {
    run_and_check_registers!(
        "
        function main() {
            let x = 42;
            print(if x == 42 { 99 } else { 0 });
        }
        ",
        99.into()
    );
}

#[test]
pub fn float_addition() {
    run_and_check_registers!(
        "
        function main() {
            let x = 1.5 + 2.5;
            print(x);
        }
        ",
        4.0f64.into()
    );
}

#[test]
pub fn float_sqrt() {
    run_and_check_registers!(
        "
        function main() {
            let x = float(144).sqrt();
            print(x);
        }
        ",
        12.0f64.into()
    );
}

#[test]
pub fn float_floor() {
    run_and_check_registers!(
        "
        function main() {
            let x = 3.9;
            print(x.floor());
        }
        ",
        3.0f64.into()
    );
}

#[test]
pub fn float_abs() {
    run_and_check_registers!(
        "
        function main() {
            let x = -7.5;
            print(x.abs());
        }
        ",
        7.5f64.into()
    );
}

#[test]
pub fn int_to_float_conversion() {
    run_and_check_registers!(
        "
        function main() {
            let x = float(42);
            print(x);
        }
        ",
        42.0f64.into()
    );
}

#[test]
pub fn float_to_int_conversion() {
    run_and_check_registers!(
        "
        function main() {
            let x = int(3.9);
            print(x);
        }
        ",
        3.into()
    );
}

#[test]
pub fn int_to_str_conversion() {
    run_and_check_registers!(
        r#"
        function main() {
            let x = str(42);
            print(x.len());
        }
        "#,
        2.into()
    );
}

#[test]
pub fn string_starts_ends_with() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello world";
            let a = s.starts_with("hello");
            let b = s.ends_with("world");
            print(a && b);
        }
        "#,
        true.into()
    );
}

#[test]
pub fn string_replace() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello world";
            let r = s.replace("world", "keel");
            print(r.len());
        }
        "#,
        10.into()
    );
}

#[test]
pub fn string_find() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello world";
            print(s.find("world"));
        }
        "#,
        6.into()
    );
}

#[test]
pub fn string_repeat() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "ab";
            print(s.repeat(3).len());
        }
        "#,
        6.into()
    );
}
#[test]
pub fn array_repeat() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = [1,2];
            let t = s.repeat(3);
            print(t.len()+t[2]);
        }
        "#,
        7.into()
    );
}

#[test]
pub fn array_contains() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [1, 2, 3, 4, 5];
            print(arr.contains(3));
        }
        ",
        true.into()
    );
}

#[test]
pub fn array_reverse() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [1, 2, 3];
            arr.reverse();
            print(arr[0]);
        }
        ",
        3.into()
    );
}

#[test]
pub fn array_remove() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [10, 20, 30];
            arr.remove(1);
            print(arr.len());
        }
        ",
        2.into()
    );
}

#[test]
pub fn array_join() {
    run_and_check_registers!(
        r#"
        function main() {
            let arr = ["a", "b", "c"];
            let s = arr.join(",");
            print(s.len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn array_modify_index() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [1, 2, 3];
            arr[1] = 99;
            print(arr[1]);
        }
        ",
        99.into()
    );
}

#[test]
pub fn break_loop() {
    run_and_check_registers!(
        "
        function main() {
            let x = 0;
            for i in 0..100 {
                if i == 5 { break; }
                x += 1;
            }
            print(x);
        }
        ",
        5.into()
    );
}

#[test]
pub fn continue_in_loop() {
    run_and_check_registers!(
        "
        function main() {
            let sum = 0;
            for i in 0..10 {
                if (i % 2) == 0 { continue; }
                sum += i;
            }
            print(sum);
        }
        ",
        25.into()
    );
}

#[test]
pub fn nested_loops() {
    run_and_check_registers!(
        "
        function main() {
            let count = 0;
            for i in 0..4 {
                for j in 0..4 {
                    count += 1;
                }
            }
            print(count);
        }
        ",
        16.into()
    );
}

#[test]
pub fn bool_and_operator() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            print(x > 3 && x < 10);
        }
        ",
        true.into()
    );
}

#[test]
pub fn bool_or_operator() {
    run_and_check_registers!(
        "
        function main() {
            let x = 15;
            print(x < 3 || x > 10);
        }
        ",
        true.into()
    );
}

#[test]
pub fn negation() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            print(-x);
        }
        ",
        (-5).into()
    );
}

#[test]
pub fn power_operator() {
    run_and_check_registers!(
        "
        function main() {
            let x = 2 ^ 10;
            print(x);
        }
        ",
        1024.into()
    );
}

#[test]
pub fn multi_arg_function() {
    run_and_check_registers!(
        "
        function add(a, b) { return a + b; }
        function main() {
            print(add(3, 4));
        }
        ",
        7.into()
    );
}

#[test]
pub fn function_called_after_loop() {
    run_and_check_registers!(
        "
        function double(n) { return n * 2; }
        function main() {
            let sum = 0;
            for i in 0..10 { sum += i; }
            print(double(sum));
        }
        ",
        90.into()
    );
}

#[test]
pub fn recursive_fn_inside_for_loop() {
    run_and_check_registers!(
        "
        function fib(n) {
            if n <= 1 { return n; }
            return fib(n-1) + fib(n-2);
        }
        function main() {
            let x = [0, 1, 2];
            let sum = 0;
            for i in x {
                sum += fib(i);
            }
            print(sum);
        }
        ",
        2.into()
    );
}

#[test]
pub fn recursive_fib_after_loop() {
    run_and_check_registers!(
        "
        function fib(n) {
            if n <= 1 { return n; }
            return fib(n - 1) + fib(n - 2);
        }
        function main() {
            let x = 0;
            for i in 0..100 { x += i; }
            print(fib(10));
        }
        ",
        55.into()
    );
}

#[test]
pub fn sieve_of_eratosthenes() {
    run_and_check_registers!(
        "
        function main() {
            let limit = 100000;
            let sieve = range(limit);
            sieve[0] = 0;
            sieve[1] = 0;
            let i = 2;
            while (i * i) <= limit {
                if sieve[i] != 0 {
                    let j = i * i;
                    while j < limit {
                        sieve[j] = 0;
                        j += i;
                    }
                }
                i += 1;
            }
            let count = 0;
            for x in sieve {
                if x != 0 { count += 1; }
            }
            print(count);
        }
        ",
        9592.into()
    );
}

#[test]
pub fn collatz_steps() {
    run_and_check_registers!(
        "
        function main() {
            let n = 27;
            let steps = 0;
            while n != 1 {
                if (n % 2) == 0 {
                    n /= 2;
                } else {
                    n = n * 3 + 1;
                }
                steps += 1;
            }
            print(steps);
        }
        ",
        111.into()
    );
}

#[test]
pub fn string_word_count() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "the quick brown fox jumps";
            let words = s.split(" ");
            print(words.len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn range_sum() {
    run_and_check_registers!(
        "
        function main() {
            let arr = range(101);
            let sum = 0;
            for x in arr {
                sum += x;
            }
            print(sum);
        }
        ",
        5050.into()
    );
}

#[test]
pub fn bubble_sort() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [5, 3, 8, 1, 9, 2, 7, 4, 6];
            let n = arr.len();
            for i in 0..n {
                for j in 0..(n - 1) {
                    if arr[j] > arr[j + 1] {
                        let tmp = arr[j];
                        arr[j] = arr[j + 1];
                        arr[j + 1] = tmp;
                    }
                }
            }
            print(arr[0]+arr[8]);
        }
        ",
        10.into()
    );
}

#[test]
pub fn quicksort() {
    run_and_check_registers!(
        r#"
        function quicksort(arr) {
            if arr.len() <= 1 {
                return arr;
            }
            let pivot = arr[0];
            let left = [];
            let right = [];
            for i in 1..arr.len() {
                if arr[i] < pivot {
                    left.push(arr[i]);
                } else {
                    right.push(arr[i]);
                }
            }
            let sorted_left = quicksort(left);
            let sorted_right = quicksort(right);
            sorted_left.push(pivot);
            for x in sorted_right {
                sorted_left.push(x);
            }
            return sorted_left;
        }
        function main() {
            let nums = [38, 27, 43, 3, 9, 82, 10];
            let sorted = quicksort(nums);
            print(sorted[0] + sorted[6]);
        }
        "#,
        85.into()
    );
}

#[test]
pub fn for_loop_called_twice() {
    run_and_check_registers!(
        "
        function sum(arr) {
            let s = 0;
            for x in arr {
                s += x;
            }
            return s;
        }
        function main() {
            sum([1, 2, 3]);
            print(sum([1, 2, 3]));
        }
        ",
        6.into()
    );
}

#[test]
pub fn two_for_loops_in_sequence() {
    run_and_check_registers!(
        "
        function main() {
            let a = [1, 2, 3];
            let b = [10, 20, 30];
            let sum = 0;
            for x in a { sum += x; }
            for x in b { sum += x; }
            print(sum);
        }
        ",
        66.into()
    );
}

#[test]
pub fn early_return_from_for_loop() {
    run_and_check_registers!(
        "
        function first_positive(arr) {
            for x in arr {
                if x > 0 { return x; }
            }
            return 0;
        }
        function main() {
            print(first_positive([-3, -1, 5, 8]));
        }
        ",
        5.into()
    );
}

#[test]
pub fn early_return_from_while_loop() {
    run_and_check_registers!(
        "
        function find(limit) {
            let i = 0;
            while i < limit {
                if i == 7 { return i; }
                i += 1;
            }
            return -1;
        }
        function main() {
            print(find(20));
        }
        ",
        7.into()
    );
}

#[test]
pub fn nested_fn_call_as_arg() {
    run_and_check_registers!(
        "
        function double(n) { return n * 2; }
        function inc(n)    { return n + 1; }
        function main() {
            print(double(inc(double(3))));
        }
        ",
        14.into()
    );
}

#[test]
pub fn multi_loop_fn_called_twice() {
    run_and_check_registers!(
        "
        function run(arr) {
            let s = 0;
            for x in arr { s += x; }
            for x in arr { s += x; }
            print(s);
        }
        function main() {
            run([1, 2, 3]);
            run([1, 2, 3]);
        }
        ",
        12.into()
    );
}

#[test]
pub fn while_fn_called_twice() {
    run_and_check_registers!(
        "
        function count_down(n) {
            let s = 0;
            while n > 0 {
                s += n;
                n -= 1;
            }
            return s;
        }
        function main() {
            count_down(5);
            print(count_down(5));
        }
        ",
        15.into()
    );
}

#[test]
pub fn function_returns_array() {
    run_and_check_registers!(
        "
        function make(n) {
            return [n, n * 2, n * 3];
        }
        function main() {
            let arr = make(4);
            print(arr[0]+arr[1]+arr[2]);
        }
        ",
        24.into()
    );
}

#[test]
pub fn pass_array_to_function() {
    run_and_check_registers!(
        "
        function last(arr) {
            let n = arr.len();
            return arr[n - 1];
        }
        function main() {
            print(last([7, 8, 9]));
        }
        ",
        9.into()
    );
}

#[test]
pub fn string_split_then_iterate() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "a,b,c,d,e";
            let parts = s.split(",");
            let count = 0;
            for p in parts { count += 1; }
            print(count);
        }
        "#,
        5.into()
    );
}

#[test]
pub fn deeply_nested_conditions() {
    run_and_check_registers!(
        "
        function classify(n) {
            if n < 0 {
                return 0;
            } else {
                if n < 10 {
                    return 1;
                } else {
                    if n < 100 {
                        return 2;
                    } else {
                        return 3;
                    }
                }
            }
        }
        function main() {
            print(classify(50));
        }
        ",
        2.into()
    );
}

#[test]
pub fn break_in_while_loop() {
    run_and_check_registers!(
        "
        function main() {
            let i = 0;
            while i < 1000 {
                if i == 42 { break; }
                i += 1;
            }
            print(i);
        }
        ",
        42.into()
    );
}

#[test]
pub fn for_loop_discard_var() {
    run_and_check_registers!(
        "
        function main() {
            let count = 0;
            for _ in [0, 0, 0, 0, 0] { count += 1; }
            print(count);
        }
        ",
        5.into()
    );
}

#[test]
pub fn int_range_loop_fn_called_twice() {
    run_and_check_registers!(
        "
        function sum_to(n) {
            let s = 0;
            for i in 0..n { s += i; }
            return s;
        }
        function main() {
            sum_to(10);
            print(sum_to(10));
        }
        ",
        45.into()
    );
}

#[test]
pub fn inc_int_to_basic() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            let y = x + 1;
            print(y);
        }
        ",
        6.into()
    );
}

#[test]
pub fn dec_int_to_basic() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            let y = x - 1;
            print(y);
        }
        ",
        4.into()
    );
}

#[test]
pub fn inc_int_commutative() {
    run_and_check_registers!(
        "
        function main() {
            let x = 10;
            let y = 1 + x;
            print(y);
        }
        ",
        11.into()
    );
}

#[test]
pub fn inc_int_to_chained() {
    run_and_check_registers!(
        "
        function main() {
            let x = 3;
            let y = x + 1;
            let z = y + 1;
            print(z);
        }
        ",
        5.into()
    );
}

#[test]
pub fn inc_int_as_function_arg() {
    run_and_check_registers!(
        "
        function identity(n) { return n; }
        function main() {
            let x = 7;
            print(identity(x + 1));
        }
        ",
        8.into()
    );
}

#[test]
pub fn dec_int_as_return_value() {
    run_and_check_registers!(
        "
        function pred(n) { return n - 1; }
        function main() {
            print(pred(20));
        }
        ",
        19.into()
    );
}

#[test]
pub fn inc_int_in_condition() {
    run_and_check_registers!(
        "
        function main() {
            let x = 9;
            let result = 0;
            if x + 1 > 9 { result = 1; }
            print(result);
        }
        ",
        1.into()
    );
}

#[test]
pub fn inc_int_does_not_mutate_source() {
    run_and_check_registers!(
        "
        function main() {
            let x = 41;
            let y = x + 1;
            print(x);
        }
        ",
        41.into()
    );
}

#[test]
pub fn dec_int_does_not_mutate_source() {
    run_and_check_registers!(
        "
        function main() {
            let x = 41;
            let y = x - 1;
            print(x);
        }
        ",
        41.into()
    );
}

#[test]
pub fn int_wraps_on_overflow() {
    run_and_check_registers!(
        "
        function main() {
            let x = 2147483647;
            x += 1;
            print(x);
        }
        ",
        (-2147483648_i32).into()
    );
}

#[test]
pub fn int_wraps_on_underflow() {
    run_and_check_registers!(
        "
        function main() {
            let x = -2147483648;
            x -= 1;
            print(x);
        }
        ",
        2147483647_i32.into()
    );
}

#[test]
pub fn negative_int_literal() {
    run_and_check_registers!(
        "
        function main() {
            let x = -2147483648;
            print(x);
        }
        ",
        (-2147483648_i32).into()
    );
}

#[test]
pub fn string_exactly_6_chars() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "abcdef";
            print(s.len());
        }
        "#,
        6.into()
    );
}

#[test]
pub fn string_exactly_7_chars() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "abcdefg";
            print(s.len());
        }
        "#,
        7.into()
    );
}

#[test]
pub fn string_small_to_large_concat() {
    run_and_check_registers!(
        r#"
        function main() {
            let a = "abc";
            let b = "defgh";
            let c = a + b;
            print(c.len());
        }
        "#,
        8.into()
    );
}

#[test]
pub fn string_escape_newline() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "a\nb";
            print(s.len());
        }
        "#,
        3.into()
    );
}

#[test]
pub fn string_escape_tab() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "a\tb";
            print(s.len());
        }
        "#,
        3.into()
    );
}

#[test]
pub fn string_escape_backslash() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "a\\b";
            print(s.len());
        }
        "#,
        3.into()
    );
}

#[test]
pub fn string_escape_quote() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "say \"hello\"";
            print(s.len());
        }
        "#,
        11.into()
    );
}

#[test]
pub fn empty_range_for_loop() {
    run_and_check_registers!(
        "
        function main() {
            let count = 99;
            for _ in 0..0 { count += 1; }
            print(count);
        }
        ",
        99.into()
    );
}

#[test]
pub fn while_never_executes() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            while x > 10 { x += 1; }
            print(x);
        }
        ",
        5.into()
    );
}

#[test]
pub fn break_only_breaks_inner_loop() {
    run_and_check_registers!(
        "
        function main() {
            let outer = 0;
            for i in 0..3 {
                for j in 0..100 {
                    if j == 2 { break; }
                }
                outer += 1;
            }
            print(outer);
        }
        ",
        3.into()
    );
}

#[test]
pub fn empty_array_len() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [];
            print(arr.len());
        }
        ",
        0.into()
    );
}

#[test]
pub fn empty_array_iteration() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [];
            let count = 0;
            for _ in arr { count += 1; }
            print(count);
        }
        ",
        0.into()
    );
}

#[test]
pub fn single_element_array_len() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [42];
            print(arr.len());
        }
        ",
        1.into()
    );
}

#[test]
pub fn array_after_all_removes() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [1, 2, 3];
            arr.remove(0);
            arr.remove(0);
            arr.remove(0);
            print(arr.len());
        }
        ",
        0.into()
    );
}

#[test]
pub fn mutual_recursion() {
    run_and_check_registers!(
        "
        function is_even(n) {
            if n == 0 { return true; }
            return is_odd(n - 1);
        }
        function is_odd(n) {
            if n == 0 { return false; }
            return is_even(n - 1);
        }
        function main() {
            print(is_even(10));
        }
        ",
        true.into()
    );
}

#[test]
pub fn null_literal_store_and_compare() {
    run_and_check_registers!(
        "
        function main() {
            let x = null;
            print(x == null);
        }
        ",
        true.into()
    );
}

#[test]
pub fn null_literal_as_default() {
    run_and_check_registers!(
        "
        function main() {
            let result = null;
            result = 42;
            print(result);
        }
        ",
        42.into()
    );
}

#[test]
pub fn array_push_type_inference_propagation() {
    run_and_check_registers!(
        "
        function build_sieve(limit) {
            let sieve = range(limit);
            sieve[0] = 0;
            sieve[1] = 0;
            let i = 2;
            while (i * i) <= limit {
                if sieve[i] != 0 {
                    let j = i * i;
                    while j < limit {
                        sieve[j] = 0;
                        j += i;
                    }
                }
                i += 1;
            }
            return sieve;
        }

        function collect_primes(sieve) {
            let primes = [];
            for x in sieve {
                if x != 0 {
                    primes.push(x);
                }
            }
            return primes;
        }

        function largest_gap(primes) {
            let max = 0;
            let i = 1;
            while i < primes.len() {
                let gap = primes[i] - primes[i - 1];
                if gap > max {
                    max = gap;
                }
                i += 1;
            }
            return max;
        }

        function main() {
            let primes = collect_primes(build_sieve(50));
            print(largest_gap(primes));
        }
        ",
        6.into()
    );
}

#[test]
pub fn split_result_survives_string_gc() {
    let text = "a abcdefghijk ".repeat(140);
    run_and_check_registers!(
        &format!(
            r#"
            function longest_word(words) {{
                let longest = "";
                for word in words {{
                    if word.len() > longest.len() {{
                        longest = word;
                    }}
                }}
                return longest;
            }}

            function main() {{
                let text = "{text}";
                let words = text.split(" ");
                print(longest_word(words).len());
            }}
        "#
        ),
        11.into()
    );
}

#[test]
pub fn expr_eval_mutual_recursion() {
    run_and_check_registers!(
        r#"
        function is_digit(c) {
            return c == "0" || c == "1" || c == "2" || c == "3" || c == "4" || c == "5" || c == "6" || c == "7" || c == "8" || c == "9";
        }
        function digit_value(c) {
            if c == "0" { return 0; } if c == "1" { return 1; } if c == "2" { return 2; }
            if c == "3" { return 3; } if c == "4" { return 4; } if c == "5" { return 5; }
            if c == "6" { return 6; } if c == "7" { return 7; } if c == "8" { return 8; }
            return 9;
        }
        function skip_spaces(expr, pos) {
            while pos < expr.len() && expr[pos] == " " { pos += 1; }
            return pos;
        }
        function parse_number(expr, pos) {
            let value = 0;
            while pos < expr.len() && is_digit(expr[pos]) {
                value = value * 10 + digit_value(expr[pos]);
                pos += 1;
            }
            return [value, pos];
        }
        function parse_factor(expr, pos) {
            pos = skip_spaces(expr, pos);
            let c = expr[pos];
            if c == "(" {
                let parsed = parse_expr(expr, pos + 1);
                let value = parsed[0];
                pos = skip_spaces(expr, parsed[1]);
                return [value, pos + 1];
            }
            if c == "-" {
                let parsed = parse_factor(expr, pos + 1);
                return [0 - parsed[0], parsed[1]];
            }
            return parse_number(expr, pos);
        }
        function parse_term(expr, pos) {
            let parsed = parse_factor(expr, pos);
            let value = parsed[0];
            pos = parsed[1];
            while pos < expr.len() {
                pos = skip_spaces(expr, pos);
                if pos >= expr.len() { break; }
                let op = expr[pos];
                if op != "*" && op != "/" && op != "%" { break; }
                parsed = parse_factor(expr, pos + 1);
                if op == "*" { value = value * parsed[0]; }
                if op == "/" { value = value / parsed[0]; }
                if op == "%" { value = value % parsed[0]; }
                pos = parsed[1];
            }
            return [value, pos];
        }
        function parse_expr(expr, pos) {
            let parsed = parse_term(expr, pos);
            let value = parsed[0];
            pos = parsed[1];
            while pos < expr.len() {
                pos = skip_spaces(expr, pos);
                if pos >= expr.len() { break; }
                let op = expr[pos];
                if op != "+" && op != "-" { break; }
                parsed = parse_term(expr, pos + 1);
                if op == "+" { value += parsed[0]; }
                if op == "-" { value -= parsed[0]; }
                pos = parsed[1];
            }
            return [value, pos];
        }
        function eval_expr(expr) { return parse_expr(expr, 0)[0]; }
        function main() {
            let expressions = [
                "17 + 5 * (31 - 12) + 144 / 3 - 8 % 5",
                "((42 + 18) * 7 - 91) / 3 + 12 * (6 + 5)",
                "1000 - (35 * 17) + (256 / 8) * (19 - 4)",
                "-18 + 7 * (8 + 9 * (12 - 5)) - 64 / 4",
                "9 * 9 * 9 - (123 + 45) / 6 + 77 % 10",
                "(314 - 159) * (26 + 53) / 5 - 97",
                "12345 % 97 + 88 * (14 - 6) - 432 / 9",
                "7 + 11 * (13 + 17 * (19 - 23 + 29))",
                "(81 / 9 + 64 / 8) * (45 - 32) + 99",
                "2048 / 4 / 4 + 33 * (21 - 8) - 17"
            ];
            let checksum = 0;
            for i in 0..8000 {
                for expr in expressions {
                    checksum += eval_expr(expr) + (i % 17);
                }
            }
            print(checksum);
        }
        "#,
        90023650.into()
    );
}

#[test]
pub fn fn_call_in_if_and_in_nested_for() {
    run_and_check_registers!(
        r#"
        function is_digit(c) {
            return c == "0" || c == "1" || c == "2" || c == "3" || c == "4" ||
                   c == "5" || c == "6" || c == "7" || c == "8" || c == "9";
        }

        function main() {
            let sum = 0;
            for i in 0..2 {
                for x in "3 + 4" {
                    if x != " " && is_digit(x) {
                        sum += int(x);
                    }
                }
            }
            print(sum);
        }
        "#,
        14.into()
    );
}

#[test]
pub fn branch_without_return() {
    run_and_check_registers!(
        "
        function choose(x) {
            if x > 0 {
                let unused = 1;
            }
            return 7;
        }

        function main() {
            print(choose(1));
        }
        ",
        7.into()
    );
}

#[test]
pub fn unusued_branch_wth_return() {
    run_and_check_registers!(
        "
        function choose(x) {
            if x > 0 {
                return 1;
            }
            return 2;
        }

        function main() {
            print(choose(0));
        }
        ",
        2.into()
    );
}

#[test]
pub fn unreachable_return_after_exhaustive_condition() {
    run_and_check_registers!(
        "
        function choose(x) {
            if x > 0 {
                return 1;
            } else {
                return 2;
            }
            return \"bad\";
        }

        function main() {
            print(choose(1));
        }
        ",
        1.into()
    );
}

#[test]
#[should_panic]
pub fn partial_return_flow_with_null() {
    run!(
        r#"
        function test(n) {
            if n == "" {
                return n;
            }
        }

        function main() {
            print(test(input("> ")).uppercase());
        }
        "#
    );
}

#[test]
pub fn unused_nested_partial_branch() {
    run_and_check_registers!(
        r#"
        function label(n) {
            if n > 0 {
                if n == 1 {
                    return "one";
                }
            }
            return "other";
        }

        function main() {
            print(label(2).uppercase());
        }
        "#,
        crate::Data::small_str("OTHER")
    );
}

#[test]
pub fn return_flow_exhaustive_condition_ignores_later_conflicting_return() {
    run_and_check_registers!(
        r#"
        function choose(n) {
            if n == 0 {
                return 10;
            } else if n == 1 {
                return 20;
            } else {
                return 30;
            }
            return "bad";
        }

        function main() {
            print(choose(2) + 1);
        }
        "#,
        31.into()
    );
}

#[test]
#[should_panic]
pub fn return_flow_return_inside_for_loop_is_not_total() {
    run!(
        r#"
        function first_word(words) {
            for word in words {
                return word;
            }
        }

        function main() {
            print(first_word(["hello"]).uppercase());
        }
        "#
    );
}

#[test]
#[should_panic]
pub fn return_flow_return_inside_while_loop_is_not_total() {
    run!(
        r#"
        function maybe_word(n) {
            while n > 0 {
                return "word";
            }
        }

        function main() {
            print(maybe_word(0).uppercase());
        }
        "#
    );
}

#[test]
#[should_panic]
pub fn return_flow_branch_returns_null() {
    run!(
        "
        function maybe_number(n) {
            if n > 0 {
                return;
            }
            return 1;
        }

        function main() {
            print(maybe_number(0) + 1);
        }
        "
    );
}

#[test]
pub fn return_flow_branch_local_return_value_type_is_preserved() {
    run_and_check_registers!(
        r#"
        function word(n) {
            if n > 0 {
                let value = "branch";
                return value;
            }
            return "fallback";
        }

        function main() {
            print(word(1).uppercase());
        }
        "#,
        crate::Data::small_str("BRANCH")
    );
}

#[test]
pub fn match_basic_int() {
    run_and_check_registers!(
        "
        function main() {
            let x = 2;
            let result = 0;
            match x {
                1 => { result = 10; }
                2 => { result = 20; }
                3 => { result = 30; }
            }
            print(result);
        }
        ",
        20.into()
    );
}

#[test]
pub fn match_with_wildcard() {
    run_and_check_registers!(
        r#"
        function main() {
            let x = "other";
            let result = 0;
            match x {
                "hello" => { result = 1; }
                "goodbye" => { result = 2; }
                _ => { result = 99; }
            }
            print(result);
        }
        "#,
        99.into()
    );
}

#[test]
pub fn match_first_arm() {
    run_and_check_registers!(
        "
        function main() {
            let x = 1;
            let result = 0;
            match x {
                1 => { result = 100; }
                2 => { result = 200; }
            }
            print(result);
        }
        ",
        100.into()
    );
}

#[test]
pub fn match_no_match() {
    run_and_check_registers!(
        "
        function main() {
            let x = 99;
            let result = 0;
            match x {
                1 => { result = 10; }
                2 => { result = 20; }
            }
            print(result);
        }
        ",
        0.into()
    );
}

#[test]
pub fn match_string_arms() {
    run_and_check_registers!(
        r#"
        function main() {
            let cmd = "run";
            let code = 0;
            match cmd {
                "stop" => { code = 1; }
                "run" => { code = 2; }
                "pause" => { code = 3; }
                _ => { code = -1; }
            }
            print(code);
        }
        "#,
        2.into()
    );
}

#[test]
pub fn match_arm_computation() {
    run_and_check_registers!(
        "
        function main() {
            let x = 3;
            let result = 0;
            match x {
                1 => {
                    result = 10 + 5;
                }
                3 => {
                    let a = 7;
                    let b = 8;
                    result = a * b;
                }
            }
            print(result);
        }
        ",
        56.into()
    );
}

#[test]
pub fn loop_break() {
    run_and_check_registers!(
        "
        function main() {
            let i = 0;
            loop {
                i += 1;
                if i == 10 { break; }
            }
            print(i);
        }
        ",
        10.into()
    );
}

#[test]
pub fn loop_continue() {
    run_and_check_registers!(
        "
        function main() {
            let i = 0;
            let sum = 0;
            loop {
                i += 1;
                if i > 20 { break; }
                if (i % 2) == 0 { continue; }
                sum += i;
            }
            print(sum);
        }
        ",
        100.into()
    );
}

#[test]
pub fn nested_loop_blocks() {
    run_and_check_registers!(
        "
        function main() {
            let count = 0;
            let i = 0;
            loop {
                i += 1;
                if i > 3 { break; }
                let j = 0;
                loop {
                    j += 1;
                    if j > 4 { break; }
                    count += 1;
                }
            }
            print(count);
        }
        ",
        12.into()
    );
}

#[test]
pub fn nested_loop_inner_break() {
    run_and_check_registers!(
        "
        function main() {
            let outer = 0;
            let i = 0;
            loop {
                i += 1;
                if i > 3 { break; }
                let j = 0;
                loop {
                    j += 1;
                    if j > 1 { break; }
                }
                outer += 1;
            }
            print(outer);
        }
        ",
        3.into()
    );
}

#[test]
pub fn nested_fn() {
    run_and_check_registers!(
        "
        function main() {
            function add(a, b) {
                return a + b;
            }
            print(add(3, 4));
        }
        ",
        7.into()
    );
}

#[test]
pub fn nested_fn_in_loop() {
    run_and_check_registers!(
        "
        function main() {
            function square(n) {
                return n * n;
            }
            let sum = 0;
            for i in 1..5 {
                sum += square(i);
            }
            print(sum);
        }
        ",
        30.into()
    );
}

#[test]
pub fn block_scope() {
    run_and_check_registers!(
        "
        function main() {
            let x = 1;
            {
                let y = 2;
                x = x + y;
            }
            print(x);
        }
        ",
        3.into()
    );
}

#[test]
pub fn range_two_arg() {
    run_and_check_registers!(
        "
        function main() {
            let arr = range(5, 10);
            let sum = 0;
            for x in arr { sum += x; }
            print(sum);
        }
        ",
        35.into()
    );
}

#[test]
pub fn range_two_arg_index() {
    run_and_check_registers!(
        "
        function main() {
            let arr = range(3, 7);
            print(arr[0]);
        }
        ",
        3.into()
    );
}

#[test]
pub fn string_uppercase() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello";
            print(s.uppercase().len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn string_lowercase() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "ABCDE";
            print(s.lowercase().len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn string_is_float() {
    run_and_check_registers!(
        r#"
        function main() {
            print("3.14".is_float());
        }
        "#,
        true.into()
    );
}

#[test]
pub fn string_is_float_false() {
    run_and_check_registers!(
        r#"
        function main() {
            print("42".is_float());
        }
        "#,
        false.into()
    );
}

#[test]
pub fn string_is_int_true() {
    run_and_check_registers!(
        r#"
        function main() {
            print("42".is_int());
        }
        "#,
        true.into()
    );
}

#[test]
pub fn string_is_int_false() {
    run_and_check_registers!(
        r#"
        function main() {
            print("hello".is_int());
        }
        "#,
        false.into()
    );
}

#[test]
pub fn string_trim_sequence() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "--hello--";
            print(s.trim_sequence("-").len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn string_trim_sequence_left() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "--hello";
            print(s.trim_sequence_left("-").len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn string_trim_sequence_right() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello--";
            print(s.trim_sequence_right("-").len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn float_round() {
    run_and_check_registers!(
        "
        function main() {
            let x = 3.7;
            print(x.round());
        }
        ",
        4.0f64.into()
    );
}

#[test]
pub fn int_abs() {
    run_and_check_registers!(
        "
        function main() {
            let x = -42;
            print(x.abs());
        }
        ",
        42.into()
    );
}

#[test]
pub fn string_reverse_method() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "abcde";
            let r = s.reverse();
            print(r.len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn array_find() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [10, 20, 30, 40];
            print(arr.find(30));
        }
        ",
        2.into()
    );
}

#[test]
pub fn array_find_missing() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [10, 20, 30];
            print(arr.find(99));
        }
        ",
        (-1).into()
    );
}

#[test]
pub fn array_sort_floats() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [3.1, 1.4, 2.7];
            arr.sort();
            print(arr[0]);
        }
        ",
        1.4f64.into()
    );
}

#[test]
pub fn array_sort_strings() {
    run_and_check_registers!(
        r#"
        function main() {
            let arr = ["banana", "apple", "cherry"];
            arr.sort();
            print(arr[0].len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn nested_array_index() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [[1, 2], [3, 4], [5, 6]];
            print(arr[1][1]);
        }
        ",
        4.into()
    );
}

#[test]
pub fn nested_array_set() {
    run_and_check_registers!(
        "
        function main() {
            let arr = [[1, 2], [3, 4]];
            arr[0][1] = 99;
            print(arr[0][1]);
        }
        ",
        99.into()
    );
}

#[test]
pub fn bool_from_string_true() {
    run_and_check_registers!(
        r#"
        function main() {
            print(bool("true"));
        }
        "#,
        true.into()
    );
}

#[test]
pub fn bool_from_string_false() {
    run_and_check_registers!(
        r#"
        function main() {
            print(bool("false"));
        }
        "#,
        false.into()
    );
}

#[test]
pub fn the_answer() {
    run_and_check_registers!(
        "
        function main() {
            print(the_answer());
        }
        ",
        42.into()
    );
}

#[test]
pub fn compound_mul_assign() {
    run_and_check_registers!(
        "
        function main() {
            let x = 5;
            x *= 3;
            print(x);
        }
        ",
        15.into()
    );
}

#[test]
pub fn compound_div_assign() {
    run_and_check_registers!(
        "
        function main() {
            let x = 20;
            x /= 4;
            print(x);
        }
        ",
        5.into()
    );
}

#[test]
pub fn compound_mod_assign() {
    run_and_check_registers!(
        "
        function main() {
            let x = 17;
            x %= 5;
            print(x);
        }
        ",
        2.into()
    );
}

#[test]
pub fn compound_pow_assign() {
    run_and_check_registers!(
        "
        function main() {
            let x = 2;
            x ^= 8;
            print(x);
        }
        ",
        256.into()
    );
}

#[test]
pub fn string_index() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello";
            print(s[0] == "h");
        }
        "#,
        true.into()
    );
}

#[test]
pub fn string_set_index() {
    run_and_check_registers!(
        r#"
        function main() {
            let s = "hello";
            s[0] = "H";
            print(s.len());
        }
        "#,
        5.into()
    );
}

#[test]
pub fn not_equal() {
    run_and_check_registers!(
        "
        function main() {
            print(3 != 4);
        }
        ",
        true.into()
    );
}

#[test]
pub fn equal_false() {
    run_and_check_registers!(
        "
        function main() {
            print(3 == 4);
        }
        ",
        false.into()
    );
}

#[test]
pub fn array_join_no_sep() {
    run_and_check_registers!(
        r#"
        function main() {
            let arr = ["a", "b", "c"];
            let s = arr.join();
            print(s.len());
        }
        "#,
        3.into()
    );
}

#[test]
pub fn float_div_zero() {
    run_and_check_registers!(
        "
        function main() {
            let x = 1.0 / 0.0;
            print(x > 9999999.0);
        }
        ",
        true.into()
    );
}

#[test]
pub fn float_negative_pow() {
    run_and_check_registers!(
        "
        function main() {
            let x = 2.0 ^ -1.0;
            print(x);
        }
        ",
        0.5f64.into()
    );
}

#[test]
pub fn float_negative_pow_square() {
    run_and_check_registers!(
        "
        function main() {
            let x = 4.0 ^ -0.5;
            print(x);
        }
        ",
        0.5f64.into()
    );
}

#[test]
pub fn type_int() {
    run_and_check_registers!(
        r#"
        function main() {
            print(type(42) == "Integer");
        }
        "#,
        true.into()
    );
}

#[test]
pub fn type_string() {
    run_and_check_registers!(
        r#"
        function main() {
            print(type("hello") == "String");
        }
        "#,
        true.into()
    );
}

#[test]
pub fn type_float() {
    run_and_check_registers!(
        r#"
        function main() {
            print(type(3.14) == "Float");
        }
        "#,
        true.into()
    );
}

#[test]
pub fn type_bool() {
    run_and_check_registers!(
        r#"
        function main() {
            print(type(true) == "Boolean");
        }
        "#,
        true.into()
    );
}
