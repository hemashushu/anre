# Building a Regular Expression Engine

![Banner](images/banner.png)

This short book explains how to build a backtracking regular expression engine, including matching, capturing groups, backreferences, and assertions. It focuses on an executable graph of nodes and transitions rather than formal automata theory.

The goal is to make the design approachable while explaining the details needed to implement it.

<!-- @import "[TOC]" {cmd="toc" depthFrom=2 depthTo=4 orderedList=false} -->

<!-- code_chunk_output -->

- [1 Why Understand the Engine?](#1-why-understand-the-engine)
- [2 A Small Pattern Language](#2-a-small-pattern-language)
- [3 The Matching Process](#3-the-matching-process)
- [4 Transitions and Nodes](#4-transitions-and-nodes)
  - [4.1 Character Sets](#41-character-sets)
  - [4.2 Repetition](#42-repetition)
  - [4.3 Optional Matching](#43-optional-matching)
  - [4.4 Concatenation](#44-concatenation)
  - [4.5 Capturing Groups](#45-capturing-groups)
  - [4.6 Backreferences](#46-backreferences)
  - [4.7 Alternation](#47-alternation)
  - [4.8 Boundary Assertions](#48-boundary-assertions)
  - [4.9 Lookahead and Lookbehind Assertions](#49-lookahead-and-lookbehind-assertions)
- [5 Implementation](#5-implementation)

<!-- /code_chunk_output -->


## 1 Why Understand the Engine?

Developers often use regular expressions to search and validate text, yet their compact syntax can be difficult to read. Copying a pattern from the internet may solve an immediate problem without explaining why it works or when it will fail.

Syntax references explain what each construct means, but not necessarily how an engine executes it. Understanding execution makes it easier to reason about matching, backtracking, and performance.

Building an engine is one way to develop that understanding.

## 2 A Small Pattern Language

A regular expression is a pattern language built from literals, repetition, groups, alternation, and assertions. It describes sequences of characters rather than general-purpose computations. Repetition and alternation provide looping and branching behavior during matching.

This engine also supports backreferences, which can match languages beyond those described by formal regular expressions. Here, "regular expression" refers to the practical pattern language, including those extensions.

> An engine searches strings for matching character sequences and can capture selected parts of each match.

For Unicode text, this engine treats a character as a Unicode scalar value, not necessarily a complete displayed symbol. Internally, it tracks UTF-8 byte offsets. The examples below use ASCII, so character indices and byte offsets coincide.

## 3 The Matching Process

Start with a single character literal: the regex `a` matches the character 'a'. A search tries the pattern at successive starting positions until one attempt succeeds or all eligible positions have been tried.

Consider these examples:

- In `"abc"`, the first character is 'a', so the attempt at index 0 succeeds.

```diagram
abc
^
|-- the processor is looking for 'a', and it is found at index 0
```

The result is a half-open range `(start, end)`: the start is included and the end is excluded. Here, `(0, 1)` identifies the character at index 0.

- In `"cat"`, the first character is 'c', so the attempt at index 0 fails.

```diagram
cat
^
|-- the processor is looking for 'a', but it is not found at index 0
```

The search then retries at index 1 and finds 'a'.

```diagram
cat
 ^
 |-- the processor is looking for 'a', and it is found at index 1
```

This attempt succeeds, returning `(1, 2)`.

- In `"dog"`, each candidate starting position is tried in turn.

```diagram
dog
^
|-- the processor is looking for 'a', but it is not found at index 0
```

None contains 'a', so the search fails. The pseudocode represents "no match" as `null`; the Rust API uses `None`.

```diagram
dog
  ^
  |-- the processor is looking for 'a', but it is not found at index 2
```

The diagrams use a cursor to show the current input position. Conceptually, a matching context tracks the search range and cursor:

```diagram
(range_start, range_end, cursor)
```

The range usually spans the whole input, from 0 to its byte length. Subsequent searches and lookaround evaluations may start elsewhere. Consuming transitions cannot read past `range_end`, but zero-width expressions may still match at that position, including on empty input.

Keep the candidate start separate from the cursor. Each attempt begins with the cursor at the candidate start. Matching may advance it, and backtracking may restore an earlier position. If the entire attempt fails, the search advances the candidate start by one Unicode scalar value and retries. The search fails only after every eligible start, including the end position, has been tried. Anchored patterns need not retry at later positions.

To find all non-overlapping matches, an iterator repeats the search from the previous match's end, not one character beyond it. For `a` in `"banana"`, this produces `[(1, 2), (3, 4), (5, 6)]`. Empty matches need special handling: skip an empty match at the previous match's end and advance by one complete UTF-8 character before retrying. This prevents duplicate empty matches and infinite loops while allowing a final empty match at the end of the input.

> A match attempt runs the pattern at one position. A search tries candidate positions; an iterator performs successive searches.

Now consider the regex `abc`, which matches a sequence of three characters. At a given start, all three must match in order and without gaps.

Consider these examples:

- In `"abcde"`, the attempt at index 0 expects 'a'.

```diagram
abcde
^
|-- the processor is looking for 'a', and it is found at index 0
```

The character matches. The processor records the start at index 0, advances to index 1, and expects 'b'.

```diagram
|-- match start, index 0
v
abcde
 ^
 |-- the processor is looking for 'b', and it is found at index 1
```

Next, 'c' matches at index 2. The cursor advances to index 3, and the attempt succeeds with `(0, 3)`.

```diagram
|-- match start, index 0
v
abcde
  ^
  |-- the processor is looking for 'c', and it is found at index 2
```

- In `"about"`, 'a' and 'b' match at indices 0 and 1, but index 2 contains 'o' rather than 'c'. The attempt fails, and its partial result is discarded; the source text itself is unchanged.

```diagram
|-- match start, index 0
v
about
  ^
  |-- the processor is looking for 'c', but it is not found at index 2
```

The search retries at each later candidate start. Since none matches `abc`, it returns `null`.

For literal sequences, this resembles [naive string search](https://en.wikipedia.org/wiki/String-searching_algorithm#Naive_string_search):

1. Select the input range and initialize the candidate start.
2. Try the entire pattern from that start, advancing the cursor as characters match.
3. If the pattern succeeds, return the half-open match range.
4. If the attempt fails, discard its partial state, advance the candidate start by one character, and retry. Return no match once all eligible starts have failed.

## 4 Transitions and Nodes

How does the processor know which part of the pattern to match next? In this design, it follows a compiled graph rather than moving a second cursor through the pattern text. Other engine designs may use different representations.

> The compiler parses a pattern and produces an executable representation. The processor executes it against the input. That representation may be a graph, bytecode, or native machine code.

This engine represents a compiled pattern using nodes and transitions.

Each transition connects two nodes and performs a matching operation, an assertion, or a state update. A character transition, for example, checks the next input character and advances the cursor if it matches.

The following is conceptual pseudocode, not compilable Rust. Cursor movements are measured in bytes, and `get_current_char()` returns no character at the end of the permitted input range. A transition continues to its target node only if its own condition succeeds:

```diagram
struct Transition {
    next_node: Node,
}

impl Transition {
    fn run(context) -> bool {
        if self.condition_holds(&context) {
            let new_context = ...;
            return next_node.run(new_context);
        } else {
            return false;
        }
    }
}
```

For example, a transition for `a` checks that input remains before comparing the character:

```diagram
fn run(context) -> bool {
    let Some(current_char) = context.get_current_char() else {
        return false;
    };
    if current_char == 'a' {
        let new_context = context.move_cursor_forward(1);
        return next_node.run(new_context);
    } else {
        return false;
    }
}
```

The compiler stores the expected character, such as 'a', in the transition. The Rust implementation uses shared transition-handling code; it does not generate a separate function or machine-code instruction for every literal.

Each node holds an ordered list of outgoing transitions. It tries them in order until one leads all the way to the exit node:

```diagram
struct Node {
    is_exit: bool,
    transitions: Vec<Transition>,
}

impl Node {
    fn run(context) -> bool {
        if self.is_exit {
            return true;
        }
        for transition in self.transitions {
            if transition.run(context.clone()) {
                return true;
            }
        }
        return false;
    }
}
```

Here, `context.clone()` means an independent copy of all mutable matching state, including the cursor, capture slots, and repetition counters. If a branch fails, the next branch starts from the original state. A successful branch must also make its final captures available to the caller; this boolean pseudocode omits that result plumbing. The actual engine avoids full copies by recording changes in an undo log and restoring them during backtracking.

The following diagram shows a character transition between two nodes:

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

A component exposes an entry node (`in node`) and an exit node (`out node`) so the compiler can connect it to other components. The nodes and transitions belong to the compiled graph; the component itself only needs their entry and exit indices.

> Reaching a component's exit does not necessarily mean the whole pattern has matched. Matching continues through any components that follow it.

Components can be combined. For example, `ab` can be compiled into two character components connected by a jump transition. Together, they form a sequence component:

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

> This engine can combine adjacent literals into a single string transition to reduce the number of matching steps.

A jump transition performs no check or state update. It continues at the target node without moving the input cursor:

```diagram
fn run(context) -> bool {
    return next_node.run(context);
}
```

The compiler builds larger components from smaller ones, then connects them into a complete pattern. Repetition, for example, adds looping transitions around an inner component.

The top-level component's entry and exit nodes are the program's entry and exit. Each match attempt starts at the entry node. Reaching the program's exit means the whole pattern has succeeded, though it need not cover the entire input unless the pattern requires both input boundaries.

The program's exit node is a special success case, not an ordinary node with no alternatives:

```diagram
fn run(context) -> bool {
    return true;
}
```

In summary, a match attempt starts at the entry node. Each node tries its transitions in order. A successful transition advances to its target node; if the continuation fails, the processor restores the saved state and tries the next alternative. The attempt succeeds upon reaching the program's exit and fails once no alternatives remain.

The recursive pseudocode illustrates depth-first backtracking. The Rust implementation uses an explicit stack to track pending transitions rather than recursively calling a function for each node. A stack avoids deep node-by-node recursion, but does not eliminate the potentially exponential running time of ambiguous patterns. For example, `(?:a+)+b` can explore many alternatives before failing on a long run of 'a's with no 'b'.

The following sections build more complex components using the same model.

### 4.1 Character Sets

A character-set component consumes one character if it belongs to a specified set. For example, `[abc]` uses a transition containing 'a', 'b', and 'c':

```diagram
fn run(context) -> bool {
    let Some(current_char) = context.get_current_char() else {
        return false;
    };
    if ['a', 'b', 'c'].contains(&current_char) {
        let new_context = context.move_cursor_forward(current_char.len_utf8());
        return next_node.run(new_context);
    } else {
        return false;
    }
}
```

Sets can also contain inclusive character ranges. For example, `[a-z0-9-]` matches a lowercase ASCII letter, an ASCII digit, or a hyphen:

```diagram
fn run(context) -> bool {
    let Some(current_char) = context.get_current_char() else {
        return false;
    };
    if ('a'..='z').contains(&current_char) ||
        ('0'..='9').contains(&current_char) ||
        current_char == '-' {
        let new_context = context.move_cursor_forward(current_char.len_utf8());
        return next_node.run(new_context);
    } else {
        return false;
    }
}
```

Advancing by `len_utf8()` preserves character boundaries when a set includes non-ASCII characters. A negated set such as `[^abc]` inverts the membership check, but still requires an input character; it cannot match at the end of the input.

The component has the following structure:

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

### 4.2 Repetition

Repetition matches an inner component a specified number of times: exactly twice in `a{2}`, at least twice in `a{2,}`, or between two and five times in `a{2,5}`. The general component adds four wrapper nodes and five transitions around the inner component:

```diagram
  /-------------------------------------------------------------------------\
  |                                                                         |
  |                      repetition back transition                         |
  |              /--------------------------------------------\             |
  |              |                                            |             |
  |              |                           | counter        |             |
  |              |    | jump                 | increment      |             |
  |              |    | transition           | transition     |             |
  |  in          |    |                      |                |             |
  |  node        v    v      /-----------\   v  right node    |       out   |
=====o==-------==o==-------==o in    out o==------==o|o==-----/       node  |
  |          ^   left        \-----------/           |o==--------------==o=====
  |  counter |   node       inner component                   ^             |
  |  reset   |                                                | repetition  |
  |  transition                                               | forward     |
  |                                                           | transition  |
  |                                                                         |
  \--------------------- greedy repetition component -----------------------/
```

- The entry and exit nodes connect the repetition to the rest of the pattern.
- The left node enters the inner component; the right node chooses whether to repeat or continue.
- The reset transition sets this repetition's counter to zero.
- The increment transition adds one after the inner component succeeds.
- The back transition repeats while below the maximum count. An unbounded quantifier has no count-based upper limit.
- The forward transition continues to the exit only when the count is within the allowed bounds.

In greedy repetition, the back transition is tried first. If another iteration or the rest of the pattern fails, backtracking can reduce the count and try the continuation again. For example, `a{2,4}a` in `"aaaa"` first tries four repetitions, then gives one back so the final `a` can match. Greediness is a preference, not a guarantee of the longest possible overall match.

Each repetition needs its own counter, and counter updates must be restored during backtracking, just like capture updates. Otherwise, alternatives inside the repeated component can leave incorrect counts behind.

The diagram executes the inner component at least once. A minimum of zero therefore needs an optional path around the repetition, described in the next section. A fixed count of zero skips the inner component entirely, but any capturing groups inside it must retain their indices.

Repeated components may themselves match empty text, as in `(?:a?)*`. To avoid an infinite loop, stop repeating an empty iteration once the minimum count has been met. Before that point, further empty iterations may still be needed to satisfy the minimum.

Lazy repetition, such as `a{2,5}?`, reverses the order of the two checking transitions:

```diagram
  /-------------------------------------------------------------------------\
  |                                                                         |
  |                                          | counter                      |
  |                   | jump                 | increment                    |
  |                   | transition           | transition                   |
  |  in        left   |                      |                        out   |
  |  node      node   v      /-----------\   v  right node            node  |
=====o==-------==o==-------==o in    out o==------==o|o==--------------==o=====
  |          ^   ^           \-----------/           |o==--\  ^             |
  |  counter |   |          inner component                |  | repetition  |
  |  reset   |   |                                         |  | forward     |
  |  transition  \-----------------------------------------/  | transition  |
  |                      repetition back transition                         |
  |                                                                         |
  \--------------------- lazy repetition component -------------------------/
```

The forward transition is tried first, so the component attempts to continue as soon as the minimum count is met. If the rest of the pattern fails, it can backtrack and try more repetitions. Greedy and lazy quantifiers differ in search order, not in which repetition counts they permit. For a fixed count such as `a{3}`, laziness has no effect.

### 4.3 Optional Matching

An optional component matches its inner component zero or one time, as in `a?`:

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

The entry node has two outgoing jumps: one enters the inner component, and the other bypasses it. Greedy optional matching tries the inner component first. If that path or its continuation fails, the processor restores the original state and tries the bypass.

Lazy optional matching, as in `a??`, reverses that order. It tries the bypass first, then the inner component if needed:

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

Combining an optional wrapper with positive-count repetition supports zero-based bounds. For example, `a*` can use an optional wrapper around `a+`, and `a{0,5}` can wrap `a{1,5}`. The wrapper and repetition must use the same greedy or lazy preference.

### 4.4 Concatenation

A sequence of patterns is called concatenation, not logical conjunction. For example, `0x[0-9a-fA-F]+` combines the literal prefix `0x` with one or more hexadecimal digits. The second component begins where the first ends.

The compiler connects the components with jump transitions and exposes the sequence as a group component:

```diagram
  /-------------------------------------------------------------\
  |                    jump                  other components   |
  |                  | transition          | and jumps          |
  |                  |                     |                    |
  |  /-----------\   |     /-----------\   |     /-----------\  |
=====o in    out o==-----==o in    out o==.....==o in    out o=====
  |  \-----------/         \-----------/         \-----------/  |
  |    component             component             component    |
  |                                                             |
  \----------------------- group component ---------------------/
```

All components must match in order. A failure in a later component may cause an earlier repetition or alternative to backtrack and try another path. The sequence fails only after those possibilities are exhausted. For example, `a+ab` matches `"aaab"` because `a+` can give back one character to the following `ab`.

### 4.5 Capturing Groups

Capturing groups record selected parts of a match. For example, `(\d{4})-(\d{2})-(\d{2})` captures the year, month, and day in `"2026-06-23"`. Conceptually, the result contains these byte ranges:

```diagram
[
    (0, 10),    // the whole matched string "2026-06-23"
    (0, 4),     // the first capturing group "2026"
    (5, 7),     // the second capturing group "06"
    (8, 10),    // the third capturing group "23"
]
```

Group 0 records the entire match. Explicit groups are numbered from 1 by the order of their opening parentheses. A named group also has an index. Even a pattern with no explicit captures needs a slot for group 0, although an API such as `find` may expose only that match.

This pattern checks the shape of a date, not its calendar validity: it also accepts `"2026-99-99"`. Add start and end anchors when validating the entire input rather than searching for a substring.

The context stores an optional range for each capture group. `None` means the group did not participate, which differs from capturing an empty string. For example, `(a)?b` in `"b"` leaves group 1 unmatched, whereas `(a*)b` captures an empty range for that group. Iterating over captures produces one such result per match.

The structure of the capturing group component is:

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

The capture-start and capture-end transitions update the group's slot with the current cursor position. These updates must be reversible: a failed alternative must not leave captures behind, and each new search must clear the previous search's state.

The compiler wraps the root expression in capture group 0 to record the entire match:

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

### 4.6 Backreferences

Backreferences match the exact text previously captured by a group. For example, `(\w+)-\1` matches `"hello-hello"`: `\1` repeats the text captured by group 1. In this engine, `\w` means ASCII letters, ASCII digits, or underscore, not Unicode word characters.

The component has a single backreference transition:

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

The transition reads the group's range and compares those bytes with the input at the current cursor. It must first check that enough input remains:

```diagram
fn run(context) -> bool {
    let Some((start, end)) = context.get_capture_group_slot(group_index) else {
        return next_node.run(context);
    };
    let length = end - start;
    if length > context.remaining_bytes() {
        return false;
    }
    let current_substring = context.get_current_substring(length);
    let previous_substring = context.get_source_substring(start, end);
    if current_substring == previous_substring {
        let new_context = context.move_cursor_forward(length);
        return next_node.run(new_context);
    } else {
        return false;
    }
}
```

Here, lengths and cursor movements are measured in bytes. Matching the captured bytes exactly preserves UTF-8 boundaries. This engine treats a backreference to a group that did not participate as an empty match, as shown above; other engines may reject it. This behavior is separate from rejecting an invalid group index or unknown group name when compiling the pattern.

### 4.7 Alternation

Alternation, written as `|`, chooses between patterns. For example, `a|b` matches 'a' or 'b'. The component connects two alternatives:

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

The entry node tries the left branch first. If that branch or the remainder of the pattern fails, it restores the original state and tries the right branch. For example, `(?:a|ab)c` matches `"abc"`: the first branch matches `a`, but the following `c` fails at `b`, so the processor retries with `ab`.

This is leftmost-first matching, not longest-match selection. For example, `a|ab` in `"ab"` returns `"a"`, because that branch already completes the pattern successfully.

### 4.8 Boundary Assertions

This engine supports four boundary assertions:

- `^` succeeds only at byte offset 0 of the original input, not at the start of a search subrange.
- `$` succeeds only at the end of the original input, not at the end of an arbitrary subrange.
- `\b` succeeds where exactly one of the adjacent characters is a word character (`[A-Za-z0-9_]` in this engine). A missing character at either end counts as non-word.
- `\B` succeeds wherever `\b` fails, including on empty input in this engine.

Other regex flavors may support multiline modes or allow `$` before a final newline. Those are not the semantics described here.

The diagram marks positions where the assertions succeed; quotation marks delimit the example and are not part of the input:

```diagram
 |-- beginning boundary == true
 |
 |            |-- ending boundary == true
 V            v
"....abcde...."
     ^    ^
     |    |-- word boundary == true
     |
     |-- word boundary == true
```

There are two word boundaries in `"....abcde...."`: immediately before 'a' and immediately after 'e'. Boundaries are positions between characters, not characters themselves. When examining the previous character in UTF-8, decode it from a character boundary rather than reading the byte at `cursor - 1`.

All four assertions have the same component structure:

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

A boundary assertion consumes no input. If its condition holds, execution continues at the target node with the cursor unchanged; otherwise, the current path fails.

### 4.9 Lookahead and Lookbehind Assertions

Lookaround checks whether another pattern matches at a position without consuming its text in the parent match. Positive assertions require a match; negative assertions require the absence of one.

For example, `\w+(?=ing)` matches `"play"` in `"playing"` and `"sing"` in `"singing"`, leaving the asserted `"ing"` outside the match. The lookbehind pattern `(?<=pre)\w+` matches `"fix"` in `"prefix"` and `"lude"` in `"prelude"`.

Negative assertions check the text adjacent to the current position; they do not exclude an entire word based on its spelling. For example, `\w+(?!ing)` still matches all of `"playing"`, because no `"ing"` follows the word's end. Similarly, `(?<!pre)\w+` matches all of `"prefix"` when searching from the input's beginning, because no `"pre"` precedes that position.

The following component matches an inner expression and then checks a lookahead. Only the assertion transition is zero-width; the inner component consumes input normally:

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

The lookbehind check runs before the consuming inner component:

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

Lookaround patterns compile into subroutes with their own entry and exit nodes. The processor pushes a nested routine onto the shared context, evaluates the subroute, and then resumes the parent at its original cursor. It must not search later starting positions within an assertion: `a(?=b)` requires 'b' immediately after 'a', not somewhere later in the input.

- Lookahead starts its subroute exactly at the parent cursor and may inspect the remaining input.
- Lookbehind requires a fixed consumed length in this engine. For `(?<=pre)\w+`, the processor moves backward by three Unicode scalar values and starts the subroute there. If fewer than three characters precede the cursor, the positive assertion fails and the negative assertion succeeds.

Lookbehind length is a count of Unicode scalar values, not bytes, and must exclude text inspected by nested assertions. Variable-length lookbehind is rejected by this implementation; support and restrictions vary among other regex engines. Nested lookahead inside lookbehind may inspect text beyond the parent cursor, so the implementation does not truncate its view of the input at that cursor. The fixed consumed length ensures that the outer lookbehind ends at the correct position.

For a positive assertion, subroute success allows the parent to continue; for a negative assertion, subroute failure does. Capture changes from failed paths must be undone. Successful positive assertions may expose captures even though the assertion itself consumes no input. The parent pattern succeeds only if its continuation also succeeds.

## 5 Implementation

The [Regex ANRE source](../src/lib.rs) implements this design in Rust. Both pattern syntaxes compile to the same AST and executable graph:

1. The [traditional parser](../src/traditional/parser.rs) and [ANRE parser](../src/anre/parser.rs) produce the shared [AST](../src/ast.rs). ANRE also expands macros before parsing.
2. The [compiler](../src/compiler.rs) turns the AST into [nodes, routes, and components](../src/object.rs). Lookaround patterns become separate routes.
3. [Transitions](../src/transition.rs) describe matching operations and state updates. The [processor](../src/process.rs) executes them with an explicit backtracking stack.
4. The [context](../src/context.rs) stores captures, counters, nested routines, and an undo log. Stack frames record where to roll back state when a path fails.
5. The public [Regex API](../src/regex.rs) exposes searching, capture extraction, and iteration. Its tests cover empty matches, UTF-8 boundaries, capture rollback, and nested repetition.

Read the compiler and processor together: each emitted transition has a corresponding runtime behavior. The recursive pseudocode in this book is a model of that behavior, not a literal transcription of the Rust implementation.