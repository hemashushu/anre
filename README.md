# Regex ANRE

![Banner](docs/images/banner.png)

[![Crates.io](https://img.shields.io/crates/v/regex-anre.svg)](https://crates.io/crates/regex-anre) [![Documentation](https://docs.rs/regex-anre/badge.svg)](https://docs.rs/regex-anre) [![License](https://img.shields.io/crates/l/regex-anre.svg)](https://github.com/hemashushu/regex-anre)

[Regex-anre](https://github.com/hemashushu/regex-anre) is a full-featured, zero-dependency regular expression engine that supports both standard and ANRE regular expressions.

Regex-anre provides the same API as the [Rust standard regular expression library "Rust-regex"](https://docs.rs/regex/), allowing it to be a drop-in replacement without any code changes.

<!-- @import "[TOC]" {cmd="toc" depthFrom=2 depthTo=4 orderedList=false} -->

<!-- code_chunk_output -->

- [1. Features](#1-features)
- [2. Quick Start](#2-quick-start)
  - [2.1 Find a specific pattern in a string](#21-find-a-specific-pattern-in-a-string)
  - [2.2 Match text and get each capture group](#22-match-text-and-get-each-capture-group)
  - [2.3 Validate a string](#23-validate-a-string)
- [3. Regular Expression Cheatsheet](#3-regular-expression-cheatsheet)
  - [3.1 Literals](#31-literals)
  - [3.2 Repetition](#32-repetition)
    - [3.2.1 Greedy quantifiers](#321-greedy-quantifiers)
    - [3.2.2 Lazy quantifiers](#322-lazy-quantifiers)
  - [3.3 Assertions](#33-assertions)
    - [3.3.1 Boundary Assertions](#331-boundary-assertions)
    - [3.3.2 Lookaround Assertions](#332-lookaround-assertions)
  - [3.4 Groups](#34-groups)
    - [3.4.1 Sequence](#341-sequence)
    - [3.4.2 Capture and Backreferences](#342-capture-and-backreferences)
  - [3.5 Logical Operators](#35-logical-operators)
- [4. The ANRE Language](#4-the-anre-language)
  - [4.1 Literals](#41-literals)
    - [4.1.1 Characters](#411-characters)
    - [4.1.2 Strings](#412-strings)
    - [4.1.3 Character Sets](#413-character-sets)
  - [4.2 Functions](#42-functions)
    - [4.2.1 Nested Invocations](#421-nested-invocations)
    - [4.2.2 Method-like Invocation](#422-method-like-invocation)
  - [4.3 Repetition](#43-repetition)
  - [4.4 Boundary Assertions](#44-boundary-assertions)
  - [4.5 Lookaround Assertions](#45-lookaround-assertions)
  - [4.6 Groups](#46-groups)
  - [4.7 Capture and Backreferences](#47-capture-and-backreferences)
  - [4.8 Logical Operators](#48-logical-operators)
  - [4.9 Macros](#49-macros)
- [5. Examples](#5-examples)

<!-- /code_chunk_output -->

## 1. Features

- **Lightweight**: Regex-anre is built from scratch without any dependencies, making it extremely lightweight, the size of the compiled library is 10 times less than the Rust-regex library.

- **Full-featured**: Regex-anre supports all general regular expression features, in addition to backreferences, look-ahead assertions, and look-behind assertions, which are not supported in the Rust-regex library.

- **Maintainable**: Regex-anre is designed to be easy to maintain, with a clean and modular code structure. The code is easy to read and understand, and most importantly, it is well-documented.

- **Reasonable performance**: Regex-anre is about 3 to 5 times slower than Rust-regex in text matching, but it is still reasonably fast. Moreover, Regex-anre has far faster compilation speed than Rust-regex, making it suitable for dynamic pattern creation.

- **New language support**: ANRE is a functional language designed to be easy to read and write. It can be translated one-to-one into traditional regular expressions and vice versa. They can even be mixed together, eliminating the overhead of writing complex regex expressions.

- **Compatibility**: Regex-anre provides the same API as the Rust-regex library, allowing you to directly replace the Rust-regex library in your project without any code changes.

## 2. Quick Start

Add the crate [regex_anre](https://crates.io/crates/regex-anre) to your project via the command line:

```bash
cargo add regex_anre
```

Alternatively, you can manually add it to your `Cargo.toml` file:

```toml
[dependencies]
regex_anre = "2.0.0"
```

The following demonstrates the three typical use cases of regular expressions.

### 2.1 Find a specific pattern in a string

```rust
// Using traditional regex to find hexadecimal color codes
let re = Regex::new(r"#[\da-fA-F]{6}").unwrap();

// Using ANRE
let re = Regex::from_anre("('#', [char_digit, 'a'..'f', 'A'..'F'].repeat(6))").unwrap();

let text = "The color is #ffbb33 and the background is #bbdd99.";

// Find one match
if let Some(m) = re.find(text) {
    println!("Found match: {}", m.as_str());
} else {
    println!("No match found");
}

// Find all matches
let matches: Vec<_> = re.find_iter(text).collect();
for m in matches {
    println!("Found match: {}", m.as_str());
}
```

### 2.2 Match text and get each capture group

```rust
// Using traditional regex to capture RGB components from hexadecimal color codes
let re =
    Regex::new(r"#(?<red>[\da-fA-F]{2})(?<green>[\da-fA-F]{2})(?<blue>[\da-fA-F]{2})").unwrap();

// Using ANRE
let re = Regex::from_anre(
    "
    /* ANRE supports comments, multiline and macro definitions,
     * which can make the regular expression more readable and maintainable.
     */

    (
        // Define a charset for hexadecimal digits with a macro `hex`
        define hex ([char_digit, 'a'..'f', 'A'..'F'])

        // Define a macro `two_hex` for two hexadecimal digits
        define two_hex hex.repeat(2)

        // Hexadecimal color code starts with a character `#`
        '#'

        // Capture groups for red, green, and blue components
        two_hex as red
        two_hex as green
        two_hex as blue
    )"
).unwrap();

let text = "The color is #ffbb33 and the background is #bbdd99.";

// Find one match and print capture groups
if let Some(m) = re.captures(text) {
    println!("Found match: {}", m.get(0).unwrap().as_str());
    println!("Red: {}", m.name("red").unwrap().as_str());
    println!("Green: {}", m.name("green").unwrap().as_str());
    println!("Blue: {}", m.name("blue").unwrap().as_str());
} else {
    println!("No match found");
}

// Find all matches and print capture groups
let matches: Vec<_> = re.captures_iter(text).collect();
for m in matches {
    println!("Found match: {}", m.get(0).unwrap().as_str());
    println!("Red: {}", m.name("red").unwrap().as_str());
    println!("Green: {}", m.name("green").unwrap().as_str());
    println!("Blue: {}", m.name("blue").unwrap().as_str());
}
```

### 2.3 Validate a string

```rust
// Using a traditional regex to validate a date string in the format `YYYY-MM-DD`
let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();

// Using ANRE
let re = Regex::from_anre(
    "
    /* Validate a date string in the format `YYYY-MM-DD`
     * The `is_start()` and `is_end()` functions are line start and end assertions,
     * which ensure that the entire string matches the pattern.
     */

    (
        is_start()
        char_digit.repeat(4)
        '-'
        char_digit.repeat(2)
        '-'
        char_digit.repeat(2)
        is_end()
    )",
).unwrap();

println!("{}", re.is_match("2025-04-22")); // Expected: true
println!("{}", re.is_match("04-22")); // Expected: false
```

## 3. Regular Expression Cheatsheet

The following table summarizes the patterns of regular expressions and the corresponding ANRE expressions.

### 3.1 Literals

| Regex Pattern | ANRE Expression          | Description                                            |
|---------------|--------------------------|--------------------------------------------------------|
| `a`           | `'a'`                    | Match a single character                               |
| `abc`         | `"abc"`                  | Match a series of characters in order                  |
| `[abc]`       | `['a', 'b', 'c']`        | Match any character in the set                         |
| `[a-z]`       | `['a'..'z']`             | Match any character in the range                       |
| `[a-zA-Z]`    | `['a'..'z', 'A'..'Z']`   | Match any character in the combined ranges             |
| `[^abc]`      | `!['a', 'b', 'c']`       | Match any character not in the set                     |
| `\d`          | `char_digit`             | Match any digit character (0-9)                        |
| `\D`          | `char_not_digit`         | Match any non-digit character                          |
| `\w`          | `char_word`              | Match any word character (alphanumeric or underscore)  |
| `\W`          | `char_not_word`          | Match any non-word character                           |
| `\s`          | `char_space`             | Match any whitespace character (space, tab, newline)   |
| `\S`          | `char_not_space`         | Match any non-whitespace character                     |
| `[a-f\d]`     | `['a'..'f', char_digit]` | Match any character in the set (combine ranges and predefined character classes) |
| `.`           | `char_any`               | Match any character except newline                     |

### 3.2 Repetition

#### 3.2.1 Greedy quantifiers

| Regex Pattern | ANRE Expression           | Description                              |
|---------------|---------------------------|------------------------------------------|
| `a?`          | `'a'?`                    | Match zero or one occurrence of 'a'      |
| `a+`          | `'a'+`                    | Match one or more occurrences of 'a'     |
| `a*`          | `'a'*`                    | Match zero or more occurrences of 'a'    |
| `a{n}`        | `'a'{n}`                  | Match exactly n occurrences of 'a'       |
| `a{n,}`       | `'a'{n..}`                | Match at least n occurrences of 'a'      |
| `a{m,n}`      | `'a'{m..n}`               | Match between n and m occurrences of 'a' |

#### 3.2.2 Lazy quantifiers

Lazy quantifiers match as few characters as possible while still satisfying the condition. They are denoted by a `?` after the greedy quantifier. For example, `a??` will match zero or one occurrence of 'a', but it will prefer to match zero occurrences if possible.

| Regex Pattern | ANRE Expression           | Description                              |
|---------------|---------------------------|------------------------------------------|
| `a??`         | `'a'??`                   | Match zero or one occurrence of 'a'      |
| `a+?`         | `'a'+?`                   | Match one or more occurrences of 'a'     |
| `a*?`         | `'a'*?`                   | Match zero or more occurrences of 'a'    |
| `a{n}?`       | `'a'{n}?`                 | Identical to `'a'{n}`                    |
| `a{n,}?`      | `'a'{n..}?`               | Match at least n occurrences of 'a'      |
| `a{m,n}?`     | `'a'{m..n}?`              | Match between m and n occurrences of 'a' |

Note that there is no lazy quantifier for `a{n}` because it matches exactly n occurrences, so there is no room for laziness.

### 3.3 Assertions

#### 3.3.1 Boundary Assertions

| Regex Pattern  | ANRE Expression          | Description                         |
|----------------|--------------------------|-------------------------------------|
| `^`            | `is_start()`             | Match the start of the string       |
| `$`            | `is_end()`               | Match the end of the string         |
| `\b`           | `is_bound()`             | Match a word boundary               |
| `\B`           | `is_not_bound()`         | Match a non-word boundary           |

#### 3.3.2 Lookaround Assertions

| Regex Pattern  | ANRE Expression          | Description                         |
|----------------|--------------------------|-------------------------------------|
| `a(?=...)`     | `'a'.is_before(...)`     | Positive lookahead                  |
| `a(?!...)`     | `'a'.is_not_before(...)` | Negative lookahead                  |
| `(?<=...)a`    | `'a'.is_after(...)`      | Positive lookbehind                 |
| `(?<!...)a`    | `'a'.is_not_after(...)`  | Negative lookbehind                 |

### 3.4 Groups

#### 3.4.1 Sequence

| Regex Pattern  | ANRE Expression          | Description                         |
|----------------|--------------------------|-------------------------------------|
| `abc\d+`       | `("abc", char_digit+)`   | Sequence of patterns or expressions |
| `(?:abc\d+)`   | `("abc", char_digit+)`   | Non-capturing group                 |

#### 3.4.2 Capture and Backreferences

| Regex Pattern  | ANRE Expression          | Description                         |
|----------------|--------------------------|-------------------------------------|
| `(abc)`        | `#("abc")`               | Indexed capture group               |
| `\1`           | `^1`                     | Indexed backreference               |
| `(?<name>abc)` | `"abc" as name`          | Named capture group                 |
| `\k<name>`     | `name`                   | Named backreference                 |

### 3.5 Logical Operators

| Regex Pattern  | ANRE Expression                   | Description                |
|----------------|-----------------------------------|----------------------------|
| `a\|b`         | `'a' \|\| 'b'`                    | Logical OR (alternation)   |
| `(a\|b)c`      | `('a' \|\| 'b', 'c')`             | Sequence with alternation  |
| `abc\d+\|foo`  | `("abc", char_digit+) \|\| "foo"` | Alternation with sequences |

## 4. The ANRE Language

The ANRE language is a functional language designed to be easy to read and write. It can be translated one-to-one into traditional regular expressions and vice versa.

The ANRE language is quite simple, it is composed of literals, functions, group operator, a logical `OR` operator, and identifiers.

- Literals represent the basic building blocks of regular expressions, such as characters, strings, and character sets. They are all called _expressions_ in ANRE.
- Functions represent the operations that can be performed on expressions, such as repetition. They take one or more expressions and numbers as parameters and return a _new expression_. There are also some functions that have no parameters, such as boundary assertions.
- Group operators allow us to group expressions together to form more complex patterns. Note that the group operator is mandatory if there are more than one expression at the root level.
- Logical operators allow us to combine expressions using logical `OR`.
- Identifiers are used to define macros and capture groups. They can be used as expressions after they are defined.

### 4.1 Literals

Literals are the basic expressions in ANRE. They can be characters, strings, or character sets.

#### 4.1.1 Characters

A character literal is a single character that is matched exactly. Character literals are surrounded by single quotes. Character literals can be any Unicode character, including letters, digits, symbols, and even emojis.

For example:

`'a'`, `'文'`, `'❤️'`

Character literals also support escape sequences, which allow us to represent special characters that cannot be typed directly. The following table lists the common escape sequences:

| Escape Sequence | Character         | Description     |
|-----------------|-------------------|-----------------|
| `\\`            | `\`               | Backslash       |
| `\'`            | `'`               | Single quote    |
| `\"`            | `"`               | Double quote    |
| `\n`            | Newline           | Line feed       |
| `\r`            | Carriage return   | Carriage return |
| `\t`            | Tab               | Horizontal tab  |
| '\0'            | Null character    | Null character  |
| `\u{X}`         | Unicode character | Unicode character with code point X |

Where `X` is hexadecimal digits `(0-9, a-f, A-F)` and the valid range for `X` is from `0` to `10FFFF` excluded `D800` to `DFFF`.

#### 4.1.2 Strings

A string literal is a sequence of characters that is matched exactly. String literals are surrounded by double quotes. String literals can contain any characters, including escape sequences.

For example:

`"hello world"`, `"你好，世界！"`, `"I ❤️ Rust!"`, "u{6587}\u{5b57}"

#### 4.1.3 Character Sets

A character set is a set of characters that can be matched. Character sets are represented as a list of characters and ranges surrounded by square brackets. A character set can contain individual characters, ranges of characters.

For example:

- `['a', 'b', 'c']`: matches any character that is 'a', 'b', or 'c'.
- `['a'..'z']`: matches any lowercase letter from 'a' to 'z'.
- `['0'..'9', 'a'..'z', '-']`: matches any digit, lowercase letter, or hyphen.

##### 4.1.3.1 Negated Character Sets

A negated character set matches any character that is not in the set. Negated character sets are represented by prefixing the character set with an exclamation mark `!`.

For example:

- `!['a', 'b', 'c']`: matches any character that is not 'a', 'b', or 'c'.
- `!['a'..'z']`: matches any character that is not a lowercase letter.
- `!['0'..'9', 'a'..'z', '-']`: matches any character that is not a digit, lowercase letter, or hyphen.

For a given source string `"abc123-xyz"`, the character set `['a'..'z']` will match the characters 'a', 'b', 'c', 'x', 'y', and 'z', while the negated character set `!['a'..'z']` will match the characters '1', '2', '3', and '-'.

##### 4.1.3.2 Nested Character Sets

Character sets can be nested to create more complex expressions.

The following demonstrates a nested character set:

```anre
[
    ['a'..'z', 'A'..'Z']
    ['0'..'9']
    ['+', '-', '_']
]
```

> ANRE expresses can be written in multiple lines, and comments can be added using `/* */` or `//`, which can make the expressions more readable and maintainable.

This character set combines three character sets:

- one for letters (both lowercase and uppercase)
- one for digits
- one for punctuations.

It is equivalent to `['a'..'z', 'A'..'Z', '0'..'9', '+', '-', '_']` but is more readable and maintainable.

Note that negated character sets are not allowed to be nested, for example, `[!['0'..'9']]` is not valid expression.

##### 4.1.3.3 Predefined Character Classes

ANRE also provides some predefined character classes for common sets of characters. These character classes are represented as identifiers. The following table lists the predefined character classes:

| Character Class  | Description                                             |
|------------------|---------------------------------------------------------|
| `char_digit`     | Matches any digit character (0-9)                       |
| `char_not_digit` | Matches any non-digit character                         |
| `char_word`      | Matches any word character (alphanumeric or underscore) |
| `char_not_word`  | Matches any non-word character                          |
| `char_space`     | Matches any whitespace character (space, tab, newline)  |
| `char_not_space` | Matches any non-whitespace character                    |

Predefined character classes can be also included in character sets, for example:

`[char_word, '+', '-', '_']`

But negated predefined character classes are not allowed to be included in character sets, for example, `[!char_digit]` is not valid expression.

### 4.2 Functions

ANRE provides functions to represent such as repetition and assertion operations.

For example:

`repeat('a', 3)` is a function with name `repeat` that takes an expression (a character literal 'a') and a number 3 as parameters, this function represents exactly three occurrences of 'a', it is equivalent to the regex `a{3}`.

Function invocation syntax:

`function_name(expression, args...) -> expression`

Not all functions have parameters and return values, for example, `is_start()` is a function that takes no parameters and returns `void` that represents the start of the string, it is equivalent to the regex `^`.

#### 4.2.1 Nested Invocations

If a function returns an expression, and another function takes an expression as a parameter, we can nest the function invocations together to create more complex expressions.

For example:

`optional(repeat('a', 3))` is a function invocation where the `optional` function takes another function invocation `repeat('a', 3)` as its parameter. This expression represents zero or one occurrence of exactly three 'a's, it is equivalent to the regex `(a{3})?`.

#### 4.2.2 Method-like Invocation

ANRE also supports method-like invocation syntax, where a function can be invoked as a method on an expression. For example:

`'a'.repeat(3)` is equivalent to `repeat('a', 3)`.

Method-like invocation syntax:

`expression.function_name(args...) -> expression`

Similar to nested invocations, method-like invocation can be chained together, for example, the following expressions are equivalent:

- `optional(repeat('a', 3))`
- `'a'.repeat(3).optional()`

Because of the method-like invocation is more concise and readable, it is recommended to use it when possible.

### 4.3 Repetition

Repetition allows us to match a pattern multiple times. As the previous section mentioned, ANRE provides functions to represent repetition, such as `repeat`, `repeat_from`, and `repeat_range`. Since these functions are commonly used, ANRE also provides notation forms for them, such as `*`, `+`, `?`, `{n}`, `{n..}`, and `{m..n}`.

The following table lists the repetition functions and their corresponding notation format:

| Function                  | Notation    | Description                                         |
|---------------------------|-------------|-----------------------------------------------------|
| `optional(exp)`           | `exp?`      | Match zero or one occurrence of the expression      |
| `one_or_more(exp)`        | `exp+`      | Match one or more occurrences of the expression     |
| `zero_or_more(exp)`       | `exp*`      | Match zero or more occurrences of the expression    |
| `repeat(exp, n)`          | `exp{n}`    | Match exactly n occurrences of the expression       |
| `repeat_from(exp, n)`     | `exp{n..}`  | Match at least n occurrences of the expression      |
| `repeat_range(exp, m, n)` | `exp{m..n}` | Match between m and n occurrences of the expression |

For example, for a given source string `"aa-aaa-aaaa"`:

- `"aa".repeat(2)` will match "aa" at index 0, 3, 7, and 9.
- `"aa".repeat_from(3)` will match "aaa" at index 3 and "aaaa" at index 7.
- `"aa".repeat_range(1, 3)` will match "aa" at index 0, "aaa" at index 3, and "aaa" at index 7

Since all repetition functions take an expression as the first parameter, and return a new expression, thus they support method-like chain invocation.

For example:

`"abc".repeat(2).optional()` is equivalent to `optional(repeat("aa", 2))`

The repetition functions are greedy by default, which means they will match as many characters as possible while still satisfying the condition. For example, for a given source string `"aaaa"`:

'a'.repeat_from(1) will match "aaaa" at index 0.

There are also lazy versions of the repetition functions, such as `lazy_optional` and `lazy_repeat_range`. They have the same parameters and return values as their greedy counterparts, but they match as few characters as possible while still satisfying the condition.

| Function                       | Notation     | Description                                         |
|--------------------------------|--------------|-----------------------------------------------------|
| `lazy_optional(exp)`           | `exp??`      | Match zero or one occurrence of the expression      |
| `lazy_one_or_more(exp)`        | `exp+?`      | Match one or more occurrences of the expression     |
| `lazy_zero_or_more(exp)`       | `exp*?`      | Match zero or more occurrences of the expression    |
| `lazy_repeat(exp, n)`          | `exp{n}?`    | Match exactly n occurrences of the expression       |
| `lazy_repeat_from(exp, n)`     | `exp{n..}?`  | Match at least n occurrences of the expression      |
| `lazy_repeat_range(exp, m, n)` | `exp{m..n}?` | Match between m and n occurrences of the expression |

For example, for a given source string `"aaaa"`:

'a'.lazy_repeat_from(1) will match "a" at index 0, "a" at index 1, "a" at index 2, and "a" at index 3.

Note that the laziness of a fixed repetition has no effect, thus `lazy_repeat(exp, n)` is semantically equivalent to `repeat(exp, n)`, and they are both equivalent to the notation `exp{n}`.

### 4.4 Boundary Assertions

### 4.5 Lookaround Assertions

### 4.6 Groups

### 4.7 Capture and Backreferences

### 4.8 Logical Operators

### 4.9 Macros

## 5. Examples

TODO
