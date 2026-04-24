## 6. How the Regular Expression Engine Works

![Detective](docs/images/detective.png)

In this section, I will discuss how the regular expression engine works. It covers the core functions of the engine, including matching, capturing, back-references, and backtracking. I will avoid introducing complex algorithms (like NFA and DFA).

I hope this article is unique in that it explains the regular expression engine in a way that is easy to understand for everyone, and comprehensive in that it covers all the important details of the engine.

### 6.1 Why do we need to understand the engine?

In the general impression of developers, regular expressions are used for validating, searching strings. The regular expression text is somewhat like random characters which are typed by a cat rolling on the keyboard. You may prefer searching for regular expressions on the internet and then copy and paste those cryptic strings into your code. Sometimes these expressions do not work; sometimes they work, but you do not know why.

Regular expressions are hard to master for two reasons: one is that they are designed concisely and compactly, and the other — more importantly — is that few people explain how they work. Most resources only tell you how to use them, similar to a teacher who teaches you only the syntax of C but never explains how a program actually runs on the computer.

You only know the regular expression when you can build it.

### 6.2 A simple language

The regular expression is a simple language that is a combination of literals, repetitions, groups, and alternative branches. It lacks some important features of a general-purpose programming language, such as variables, conditional branches, and loops. Its sole purpose is to describe what the expected characters look like — or more accurately, to guide the processor on how to match characters in the source string.

> In short, regular expressions are used to match and capture characters

Yes, it is not about strings, but about characters. This distinction is important because it is the foundation for understanding regular expressions and how the engine works.

### 6.3 The matching process

Let's start with the simplified matching process. The simplest regular expression is just a character literal. For example, the regex `a` will match the character 'a' in a string.

Consider these examples:

- For a given string "abc", the processor will check the first character 'a', and since it is what the processor is looking for, it will mark the position (index 0 in this case) and end the process with success.

```diagram
abc
^
|-- the processor is looking for 'a', and it is found at index 0
```

The processor will return a tuple `(start, end)`, which are the start and end positions of the matched characters. In this case, it will return `(0, 1)`, which means the character 'a' is found at index 0 and ends at index 1 (the end position is exclusive).

- For a given string "cat", the processor will check the first character 'c', and since it is not what the processor is looking for, it will discard the matched characters (which is nothing in this case) and end the process with failure.

```diagram
cat
^
|-- the processor is looking for 'a', but it is not found at index 0
```

But the processor would not stop its job, it restart the matching process starts from the position next to the last start position (which is index 1 in this case) and find the character 'a'.

```diagram
cat
 ^
 |-- the processor is looking for 'a', and it is found at index 1
```

Character 'a' is what the processor is looking for, it will mark the position (index 1 in this case) and end the process with success. Finally, the processor returns `(1, 2)`.

- For a given string "dog", the processor will check each character one by one.

```diagram
dog
^
|-- the processor is looking for 'a', but it is not found at index 0
```

Since none of them is what the processor is looking for, it end the process with failure and returns `null`.

```diagram
dog
  ^
  |-- the processor is looking for 'a', but it is not found at index 2
```

You may have notice that there is a "cursor" in the above diagrams, which is the position of the processor the currently checking. Actually, the processor has a context object, which contains some information the processor needs during matching process. One of them is a tuple:

```diagram
(range_start, range_end, cursor)
```

The `range_start` and `range_end` positions are the checking range of source string. Usually, the `range_start` position is 0 and the `range_end` position is the length of the string, but in some cases (such as lookahead and lookbehind assertions, which will be covered in the next section), the `range_start` and `range_end` positions can be different.

The `cursor` is the position the processor is currently checking, as demonstrated in the above examples. When a new matching process starts, the `cursor` is set to the `range_start` position, and it keeps moving forward as the processor checks each character. When matching fails, the processor pulls the `cursor` back to the position just after the last start position and repeats the process. When the `cursor` start position reaches the `range_end` position, it indicates that the whole process has ended with failure.

It is worth mentioning that some regular expression engines also provide functions like `find_all` or `match_all` to find all occurrences of a character in the string. The principle is quite simple: the engine repeats the matching process starting from the position just after the last successful match. For example, for a given string "banana", the engine will ultimately return `[(1, 2), (3, 4), (5, 6)]`, which contains three matches. However, this is a function of the engine, not the processor. In this article, we focus only on how the processor works.

> A processor performs the matching job only once on the source string, but the engine may launch the processor multiple times (with different `cursor` start positions) to find all matches.

Now, let's look at a bit more complex example, to match strings. For example, the regex `abc` expects to match the string "abc" in the source string.

To the engine, strings are simply treated as a sequence of characters. The processor tries to match each character in the sequence one by one; if all characters are found in the source string, the processor marks the start and end positions and ends the process with success.

Consider these examples:

- For a given string "abcde", the processor set the cursor to the beginning of the string (index 0) and expect character 'a'.

```diagram
abcde
^
|-- the processor is looking for 'a', and it is found at index 0
```

It found 'a' at index 0 is what it is looking for, so it mark the start position (index 0) and moves the cursor to the next position (index 1) and expects character 'b'.

```diagram
|-- the matched characters start position, index 0
v
abcde
 ^
 |-- the processor is looking for 'b', and it is found at index 1
```

This process continues and the last expected character 'c' is found at index 2. Now, all expected characters are found, and the processor marks the end position (index 3) and end the process with success and returns `(0, 3)`.

```diagram
|-- the matched characters start position, index 0
v
abcde
  ^
  |-- the processor is looking for 'c', and it is found at index 2
```

- For a given string "about", the processor finds the expected characters 'a' and 'b' at index 0 and index 1 respectively, but when it tries to find 'c', it finds 'o' at index 2, which is not what it is looking for. So the processor discards the matched characters ("ab" in this case) and ends the current attempt with failure.

```diagram
|-- the matched characters start position, index 0
v
about
  ^
  |-- the processor is looking for 'c', but it is not found at index 2
```

The processor does not stop immediately; it keeps advancing the cursor's start position and trying to find the "a-b-c" sequence until the cursor's start position reaches the end of the string. Since no match is found, it ends the process with failure and returns `null`.

In summary, the matching process is a bit similar to the simplest String-searching algorithm - the [naive string search](https://en.wikipedia.org/wiki/String-searching_algorithm#Naive_string_search). The complete process is:

- The processor confirm the checking range of the source string, and set the cursor to the beginning of the checking range.
- It check the character the cursor is pointing to, if it is what it is looking for, it will mark the position and move on to the next character it needed until it finds all expected characters. If all expected characters are found, the processor will mark the end position and end the process with success and return `(start_position, end_position)`.
- If any character does not match during the process, the processor discards the matched characters (if any) and ends the current attempt with failure. It then pulls the cursor back to the position just after the last start position and repeats the matching process. If the cursor's start position reaches the end of the string, the whole process ends with failure and the processor returns `null`.

### 6.4 Transitions and Nodes

After reading the previous section, you might be wondering whether there is a second cursor pointing to the current expected character in the regular expression. There is no such cursor. Since regular expressions are a "language", not "data", they are intended to be compiled into code and run as a program.

> A complete regular expression engine consists of two parts: the compiler and the processor. The compiler parses the regular expression and generates code (which can be native machine instructions, bytecode, or a special data structure, depending on the implementation) that is then executed by the processor.

From a programming perspective, regular expression programs are built from two structures: nodes and transitions.

Transitions are the paths between nodes. There are many kinds of transitions, such as character transitions, charset transitions, and repetition transitions. Most transitions resemble a function in a programming language that contains a single `if` statement, which checks the current state and determines whether it passes or not.

The pseudo code of a typical transition template is like this:

```diagram
struct Transition {
    next_node: Node,
}

impl Transition {
    fn run(context) -> bool {
        if condition is met {
            let new_context = ...;
            return next_node.run(new_context);
        }else {
            return false;
        }
    }
}
```

For example, the single character regex `a` can be compiled into a transition:

```diagram
fn run(context) -> bool {
    let current_char = context.get_current_char();
    if current_char == 'a' {
        let new_context = context.move_cursor_forward(1);
        return next_node.run(new_context);
    }else {
        return false;
    }
}
```

Transitions are generated by the compiler; code such as `if current_char == 'a'` is hard-coded. When the regular expression becomes complex, many transitions are generated.

Nodes are containers of transitions; each node holds a list of transitions. The pseudo code of a typical node is like this:

```diagram
struct Node {
    transitions: Vec<Transition>,
}

impl Node {
    fn run(context) -> bool {
        for transition in self.transitions {
            if transition.run(context) {
                return true
            }
        }
        return false
    }
}
```

The following diagram shows how the nodes and transitions are connected:

```diagram
  /-----------------------------\
  |          character          |
  |        | transition         |
  |        v                    |
=====o==-------------------==o=====
  | in node            out node |
  |                             |
  \---- character component ----/
```

We then wrap the pair of nodes (`in node` and `out node`) and the transitions into a component, which is the basic unit of the program.

> Each component has an `in node` and an `out node`; these form the interface of the component.

Components can be nested, for example, the regex `ab` can be compiled into two character components, and the two components are connected by a special transition called `jump` transition. All of them form a new component - string component. The following diagram represents the structure of string component:

```diagram
  /-----------------------------------------------\
  |                                               |
  |    character                     character    |
  |    component        jump         component    |
  |  /-----------\   transition    /-----------\  |
=====o in    out o==-------------==o in    out o=====
  |  \-----------/                 \-----------/  |
  |                                               |
  \--------------- string component --------------/
```

> Some implementations may optimize the sequence of characters into one new kind of transition, which is called string transition.

The `jump` transition is used to connect the two components, there is no checking inside this transition, it just guides the processor to jump to the next node without moving the cursor. The pseudo code of the `jump` transition is like this:

```diagram
fn run(context) -> bool {
    return next_node.run(context);
}
```

Components are the building blocks of the program, they can be simple (like the character component) or complex (like the repeatition component the next section talks about, which contains multiple transitions, nodes, and inner components). The compiler generates components for each part of the regular expression, and then connects them together to form the complete program.

Where is the entry point of the program? In general, the `in node` of the top-most component is the entry. When a new matching process starts, the processor runs the `run` function of the entry node and keeps running the program until it reaches the exit node, which is the `out node` of the top-most component. Reaching the exit node means the process succeeded.

The exit node is special, its transition list is empty, and the pseudo code is like this:

```diagram
fn run(context) -> bool {
    return true
}
```

In summary, when a regular expression program is executed, the following process is performed:

- The processor invokes the function `run` from the entry node.
- The node calls each transition in its list:
  - If a transition is passed, the transition call the function `run` of its next node.
  - If a transition is not passed, the node tries the next transition in its list.
  - If any of the transitions returns true, the node return true without trying the rest of the transitions.
  - If all transitions are tried and none of them is passed, the node returns false.
- If the current node is the exit node, it returns true directly.
- If the entry node returns true, it means the process is successful, otherwise, it means the process is failed.

This is how the processor works. I have introduced the basic components and transitions — the character component, the string component, the character transition, and the jump transition. In the following sections, I will introduce more complex components and transitions.

#### 6.4.1 Charset

The charset component is used to match a character against a set of characters. A charset component contains a single charset transition, which holds a set of characters and checks whether the current character is in that set. For example, the regex `[abc]` is compiled into a charset transition with the character set `['a', 'b', 'c']`. The pseudo code of this charset transition is like this:

```diagram
fn run(context) -> bool {
    let current_char = context.get_current_char();
    if ['a', 'b', 'c'].contains(current_char) {
        let new_context = context.move_cursor_forward(1);
        return next_node.run(new_context);
    }else {
        return false;
    }
}
```

Charset transitions can also contain character ranges. For example, the pseudo code of the transition for the regex `[a-z0-9-]` is like this:

```diagram
fn run(context) -> bool {
    let current_char = context.get_current_char();
    if ('a'..='z').contains(&current_char) ||
        ('0'..='9').contains(&current_char) ||
        current_char == '-' {
        let new_context = context.move_cursor_forward(1);
        return next_node.run(new_context);
    }else {
        return false;
    }
}
```

The structure of the charset component is:

```diagram
  /-----------------------------\
  |          charset            |
  |        | transition         |
  |        v                    |
=====o==-------------------==o=====
  | in node            out node |
  |                             |
  \------ charset component ----/
```

#### 6.4.2 Repetition

The repetition component is used to match an inner component a certain number of times, such as `a{2}`, `a{2,}`, and `a{2,5}`. A repetition component consists of 5 transitions and 4 nodes, and its structure is:

```diagram
  /-------------------------------------------------------------------------\
  |                                                                         |
  |                      repetition back transition                         |
  |              /--------------------------------------------\             |
  |              |                                            |             |
  |              |     | counter             | counter        |             |
  |              |     | save                | load & inc     |             |
  |              |     | transition          | transition     |             |
  |  in          |     |                     |                |             |
  |  node        v     v     /-----------\   v  right node    |       out   |
=====o==-------==o==-------==o in    out o==------==o|o==-----/       node  |
  |          ^   left        \-----------/           |o==--------------==o=====
  |  counter |   node       inner component                   ^             |
  |  reset   |                                                | repetition  |
  |  transition                                               | forward     |
  |                                                           | transition  |
  |                                                                         |
  \--------------------- greedy repetition component -----------------------/
```

- `in node` and `out node`: they are the interface of the component.
- `left node` and `right node`: they are used internally to connect the inner component and the repetition transitions.
- `counter reset transition`: each repetition component has a counter, which is used to count how many times the inner component is matched. The counter reset transition is used to reset the counter to 0 before the loop starts.
- `counter save transition`: this transition is used to save the current counter value to a "counter stack" before the inner component is executed, this transition is needed because the inner component may contain other repetition components, which may modify the outer counter value, so we need to save the current counter value before executing the inner component, and restore it after the inner component is executed.
- `counter load and increment transition`: this transition is used to load the counter value from the "counter stack", and increment the counter by 1, then to prepare for the checking transitions:
  - `repetition back transition`: this transition is used to check the current counter value, if it is less than the maximum repetition times, it will jump back to the `left node` to execute the inner component again, otherwise, it will return false and the `repetition forward transition` will be tried.
  - `repetition forward transition`: this transition is used to check the current counter value also, if it is greater than or equal to the minimum repetition times, it will jump to the `out node`, otherwise, the repetition component returns false.

As described above, the repetition component executes the inner component as many times as possible until the counter reaches the maximum repetition count — hence the name greedy repetition. For example, the regex `a{2,4}` is compiled into a repetition component whose inner component is the character 'a', with a minimum of 2 and a maximum of 4 repetitions; it prefers to match 4 'a' characters whenever possible.

There is another kind of repetition component called lazy repetition, such as `a{2,5}?`, which is similar to greedy repetition but prefers to match as few times as possible. The structure of the lazy repetition component is almost identical to the greedy one, except that the order of the checking transitions is reversed:

```diagram
  /-------------------------------------------------------------------------\
  |                                                                         |
  |                    | counter             | counter                      |
  |                    | save                | load & inc                   |
  |                    | transition          | transition                   |
  |  in        left    |                     |                        out   |
  |  node      node    v     /-----------\   v  right node            node  |
=====o==-------==o==-------==o in    out o==------==o|o==--------------==o=====
  |          ^   ^           \-----------/           |o==--\  ^             |
  |  counter |   |          inner component                |  | repetition  |
  |  reset   |   |                                         |  | forward     |
  |  transition  \-----------------------------------------/  | transition  |
  |                      repetition back transition                         |
  |                                                                         |
  \--------------------- lazy repetition component -------------------------/
```

As shown in the diagram above, the `repetition forward transition` in the `right node` is tried before the `repetition back transition`. As a result, the component jumps to the `out node` as soon as the counter reaches the minimum repetition count, matching as few times as possible.

#### 6.4.3 Optional

The optional component is used to match an inner component 0 or 1 time, such as `a?`. The structure of the optional component is:

```diagram
  /-----------------------------------------------------\
  |                                                     |
  |              jump |       inner       | jump        |
  |  in    transition |     component     | transition  |
  |  node             v   /-----------\   v             |
=====o|o==--------------==o in    out o==----------==o=====
  |   |o==--\             \-----------/          out ^  |
  |         |                                   node |  |
  |         \----------------------------------------/  |
  |                       jump transition               |
  |                                                     |
  \--------------- greedy optional component -----------/
```

There are two jump transitions in the `in node`: one jumps to the `inner component`, and the other jumps directly to the `out node`. It prefers to match the inner component when possible, which is why it is called greedy optional. For example, the regex `a?` is compiled into an optional component whose inner component is the character 'a', and it prefers to match 'a' whenever possible.

There is another kind of optional component called lazy optional, such as `a??`, which is similar to the greedy optional but prefers to skip the inner component whenever possible. The structure of the lazy optional component is:

```diagram
  /-----------------------------------------------------\
  |                       jump transition               |
  |         /----------------------------------------\  |
  |  in     |                            jump        |  |
  |  node   |                          | transition  |  |
=====o|o==--/         /-----------\    v             v  |
  |   |o==----------==o in    out o==--------------==o=====
  |               ^   \-----------/                out  |
  |          jump |       inner                   node  |
  |    transition |     component                       |
  |                                                     |
  \---------------- lazy optional component ------------/
```

Combined with the repetition component, these components can be used to match an inner component zero or more times, such as `a*` and `a{0,5}`.

#### 6.4.4 Conjunction

In general, a regular expression is a sequence of components. For example, `0x[0-9a-fA-F]+` is a regex that matches a hexadecimal number. It contains two components: the string component `0x` and the repetition component `[0-9a-fA-F]+`. The processor executes the components one by one.

The compiler wraps the sequence of components into a "group component", where all components are connected by jump transitions:

```diagram
  /-------------------------------------------------------------\
  |                    jump                  other components   |
  |                  | transition          | and transitions    |
  |                  |                     |                    |
  |  /-----------\   |     /-----------\   |     /-----------\  |
=====o in    out o==-----==o in    out o==.....==o in    out o=====
  |  \-----------/         \-----------/         \-----------/  |
  |    component             component             component    |
  |                                                             |
  \----------------------- group component ---------------------/
```

When any inner component returns false, the group component returns false immediately without trying the remaining components. Only when all inner components pass does the group component return true.

#### 6.4.5 Capture Groups

It is worth mentioning that the processor can not only match and return the `(start, end)` position of the entire matched characters, but also supports extracting the parts of the matched characters you are interested in — this is called capture groups. For example, the regex `(\d{4})-(\d{2})-(\d{2})` matches a date string like "2026-06-23" and contains three capture groups used to extract the year, month, and day from the matched string. And the processor will return a list of tuple:

```diagram
[
    (0, 10),    // the whole matched string "2026-06-23"
    (0, 4),     // the first capture group "2026"
    (5, 7),     // the second capture group "06"
    (8, 10),    // the third capture group "23"
]
```

In practice, the processor always returns a list of tuples even when there are no capture groups in the regex; the first tuple is always the position of the overall matched result.

> If an engine supports functions like `find_all` or `match_all` to find all matches in the source string, the engine returns a list of lists of tuples.

To support capture groups, the processor's context object holds "capture group slots" — a list of tuples, where each tuple stores the start and end position of a capture group. The processor updates these slots when it executes a capture group component, and the final result is read from these slots.

The structure of the capture group component is:

```diagram
  /-------------------------------------------------\
  |                                                 |
  |  capture start                   capture end    |
  |  transition          inner       transition     |
  |       |            component       |            |
  |       v         /-------------\    V            |
=====o==----------==o in      out o==----------==o=====
  | in              \-------------/            out  |
  | node                                      node  |
  |                                                 |
  \---------------- capture component---------------/
```

Each capture group component has a group index and an optional name. The `capture start transition` marks the start position of the capture group, and the `capture end transition` marks the end position. The processor updates the "capture group slots" when it runs these two transitions.

In general, the compiler generates a capture group component for the whole regex, thus the first capture group slot is used to store the position of the overall matched characters. The structure of program component is:

```diagram
  /---------------------------------------------------------\
  |                                                         |
  |  capture start |                     capture end |      |
  |     transition |                      transition |      |
  |                V    /-------------\              v      |
=====o==-------------===o in      out o===-------------==o=====
  |  in                 \-------------/                 out |
  |  node               root expression                node |
  |                        component                        |
  |                                                         |
  \------------------- program component--------------------/
```

#### 6.4.6 Back-references

Back-references are used to match the same characters as a previously matched capture group. For example, the regex `(\w+)-\1` matches a string containing two identical words separated by a hyphen, such as "hello-hello". Here `\1` is a back-reference to the first capture group, meaning it matches the same characters that the first capture group captured.

The structure of the back-reference component is:

```diagram
  /-----------------------------\
  |          back-reference     |
  |        | transition         |
  |        v                    |
=====o==-------------------==o=====
  | in node            out node |
  |                             |
  \-- back-reference component -/
```

The back-reference transition read the specified capture group slot to get the start and end position of the previously matched characters, then it checks if the current characters in the source string are the same as the previously matched characters. The pseudo code of the back-reference transition is like this:

```diagram
fn run(context) -> bool {
    let (start, end) = context.get_capture_group_slot(group_index);
    let length = end - start;
    let current_substring = context.get_current_substring(length);
    let previous_substring = context.get_source_substring(start, end);
    if current_substring == previous_substring {
        let new_context = context.move_cursor_forward(length);
        return next_node.run(new_context);
    }else {
        return false;
    }
}
```

#### 6.4.7 Alternative Branches

There is only one type of branch in regular expressions: the alternative branch (i.e., the logical OR operator), such as `a|b`, which matches either 'a' or 'b'. The structure of the alternative branch component is:

```diagram
  /-------------------------------------------------------\
  |                                                       |
  |            jump         left          jump            |
  |      transition |     component     | transition      |
  |                 v   /-----------\   v                 |
  |  in     /---------==o in    out o==--------\      out |
  |  node   |           \-----------/          |     node |
=====o|o==--/                                  |-----==o=====
  |   |o==--\                                  |          |
  |         |           /-----------\          |          |
  |         \---------==o in    out o==--------/          |
  |                 ^   \-----------/   ^                 |
  |            jump |      right        | jump            |
  |      transition |    component      | transition      |
  |                                                       |
  \------------ alternative branch component -------------/
```

There are two jump transitions in the `in node`: one jumps to the `left component`, and the other jumps to the `right component`. Both jump to the `out node` if they succeed, so as long as one of the components matches, the entire alternative branch matches.

#### 6.4.9 Boundary Assertions

There are 4 types of boundary assertions in regular expressions: `^`, `$`, `\b`, and `\B`.

- `^`: matches the beginning of the string. The beginning boundary assertion transition checks whether the cursor equals the start position of the checking range.
- `$`: matches the end of the string. The end boundary assertion transition checks whether the cursor equals the end position of the checking range.
- `\b`: matches a word boundary. The word boundary assertion transition checks whether the current position is a word boundary — that is, the current character is a word character (alphanumeric or underscore) and the previous character is not, or vice versa.
- `\B`: matches a non-word boundary. The non-word boundary assertion transition simply inverts the result of the word boundary assertion transition.

The structure of these boundary assertion components are the same:

```diagram
  /-----------------------------\
  |          boundary assertion |
  |        | transition         |
  |        v                    |
=====o==-------------------==o=====
  | in node            out node |
  |                             |
  \--- bound assert component --/
```

Unlike other components, the boundary assertion component does not advance the cursor. It only peeks at the source string and returns true or false.

#### 6.4.10 Lookahead and Lookbehind Assertions

Lookahead and lookbehind assertions check whether a certain pattern is followed or preceded by another pattern, without including that surrounding pattern in the matched result.

For example, the lookahead assertion `\w+(?=ing)` matches a word only if it is followed by 'ing', as in "playing" or "singing". The lookbehind assertion `(?<=pre)\w+` matches a word only if it is preceded by 'pre', as in "prefix" or "prelude".

There are also negative variants: `\w+(?!ing)` matches a word only if it is not followed by 'ing', and `(?<!pre)\w+` matches a word only if it is not preceded by 'pre'.

The structure of the look-ahead assertion component is:

```diagram
  /----------------------------------------------\
  |                                              |
  |                  inner         | lookahead   |
  |  in             component      | transition  |
  |  node         /-----------\    v             |
=====o==--------==o in  out   o==-----------==o=====
  |       jump    \-----------/             out  |
  |    transition                          node  |
  |                                              |
  \------------- lookahead assertion component --/
```

And the structure of the look-behind assertion component is:

```diagram
  /-----------------------------------------------\
  |                                               |
  |       | lookbehind                            |
  |  in   | transition                jump        |
  |  node v           /-----------\   transition  |
=====o==------------==o in  out   o==--------==o=====
  |                   \-----------/          out  |
  |                  inner component        node  |
  |                                               |
  \-------- lookbehind assertion component -------/
```

Look-around assertions are sub-programs within the regular expression program. They have their own entry and exit nodes and are executed with new context objects; only the checking range of the context object differs:

- For a lookahead assertion, the start of the checking range is the current cursor position, and the end is the same as in the original context.
- For a lookbehind assertion, the end of the checking range is the current cursor position, and the start is calculated from the maximum length of the lookbehind pattern. For example, for `(?<=pre)\w+`, the pattern "pre" has a maximum length of 3, so the start is the current cursor position minus 3. Variable-length lookbehind assertions are not supported in most regular expression engines.

The look-around assertion component returns true if the inner component matches successfully, and false otherwise.

When the sub-program end, the main program continues to execute the look-around assertion component with the return value from the sub-program.
