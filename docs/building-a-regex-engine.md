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
- [6 Performance Optimizations](#6-performance-optimizations)
  - [6.1 Where the Time Goes](#61-where-the-time-goes)
  - [6.2 Skipping Impossible Start Positions](#62-skipping-impossible-start-positions)
  - [6.3 Skipping Runs That Already Failed](#63-skipping-runs-that-already-failed)
  - [6.4 Matching a Run of Characters in One Step](#64-matching-a-run-of-characters-in-one-step)
  - [6.5 Making Each Step Cheaper](#65-making-each-step-cheaper)
  - [6.6 Measuring the Results](#66-measuring-the-results)

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

> Section 6.4 describes a faster equivalent that this engine uses for greedy repetitions of a single character.

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

## 6 Performance Optimizations

The engine described in the previous chapters is correct but slow. This chapter shows how to make it about 25 times faster while keeping its design: the compiled graph, the backtracking stack, and the undo log all stay the same.

None of these optimizations changes *which* text matches; they only change how quickly the engine finds it. Every one of them follows the same rule:

> An optimization may skip work only if we can prove that the skipped work would have failed anyway.

Keep this rule in mind. For each optimization, we first look at an example of wasted work, then at the shortcut, and finally at why the shortcut can never change the result.

### 6.1 Where the Time Goes

Let's watch the engine search for `cat` in the text `"the dog sat"`:

```diagram
t h e   d o g   s a t
^                        attempt at 0: expects 'c', finds 't', fails
  ^                      attempt at 1: expects 'c', finds 'h', fails
    ^                    attempt at 2: expects 'c', finds 'e', fails
      ^                  attempt at 3: expects 'c', finds ' ', fails
        ...              ...and so on, up to the end of the text
```

The text contains no 'c' at all, yet the engine makes one attempt per position, and each attempt does more work than the diagram suggests. It has to:

1. push the transitions of the entry node onto the stack,
2. record the start of capture group 0 (the whole match),
3. run the character transition, which fails,
4. pop the stack and roll back the capture record.

For a 2 MB text, that is about two million attempts, almost all of them hopeless.

There are two ways to go faster:

- **Make fewer attempts** by recognizing positions where a match cannot start. Sections 6.2 and 6.3 do this.
- **Make each attempt cheaper** by reducing the work per step. Sections 6.4 and 6.5 do this.

Skipping an attempt entirely is always cheaper than running it quickly, so we start with the first.

### 6.2 Skipping Impossible Start Positions

**The idea.** Consider the date pattern `\d{4}-\d{2}-\d{2}` and the text `"Born on 2026-06-23 in Paris"`. Every match must begin with a digit, so an attempt at a letter or a space is certain to fail. Only the eight digits are worth trying:

```diagram
Born on 2026-06-23 in Paris
        ^^^^ ^^ ^^              only these 8 positions can start a match
```

That is 8 attempts instead of 27, and in typical English text the saving is far greater, because digits are rare.

**The byte table.** Before the search starts, the compiler builds a table with 256 entries, one for every possible byte value. Entry `b` is true if a match may begin with byte `b`. For the date pattern, the entries for `'0'` through `'9'` are true and all others are false. During the search, the processor scans the text, looks up each byte in the table, and starts an attempt only where the entry is true. A table lookup is a single memory read, which is much cheaper than starting an attempt.

**Building the table.** The compiler walks the pattern's AST using a few simple rules:

- A literal begins with its first character: `abc` begins with `'a'`.
- A sequence begins with its first element that consumes a character. Zero-width elements such as `\b` and `^` are skipped, because they do not consume anything: `\b[A-Z][a-z]+` begins with a character from `[A-Z]`.
- An alternation may begin with whatever either branch begins with: `apple|banana` begins with `'a'` or `'b'`.
- A repetition begins with whatever its inner expression begins with, provided it repeats at least once: `(?:\d{1,3}\.){3}` begins with a digit.
- A negated set begins with any character it does not exclude: `[^>]` may begin with every byte except `'>'`.

**When the analysis gives up.** Some patterns can match empty text. For example, `a*` matches an empty string at every position, even in `"xyz"`, where it finds an empty match at position 0. If the table said "a match must begin with `'a'`", the search would skip position 0 and miss that match. So whenever the beginning of a pattern may match nothing, as in `a*`, `a?b`, or `x{0,2}y`, the compiler builds no table at all, and the search simply tries every position as before. Giving up is always safe: it only means this optimization is not used.

**Non-ASCII characters.** The table works on bytes, but text is UTF-8, where a character may occupy several bytes. The table only uses a character's first byte, called its lead byte:

```diagram
character   UTF-8 bytes     lead byte
---------   -----------     ---------
a           61              61
文          E6 96 87        E6
一          E4 B8 80        E4
龥          E9 BE A5        E9
```

For the range `[一-龥]`, the compiler marks every lead byte from `E4` to `E9`. This marks a little too much: the character `䀀` (bytes `E4 80 80`) is not in the range, but its lead byte is marked. That is harmless, because an extra candidate merely leads to an attempt that fails. The table may report too many candidates, but it must never miss a real one.

**Checking a literal prefix.** The byte table only knows the first byte. Consider `class="[a-z]+"` in an HTML document. Every `'c'` is a candidate, but most belong to words such as `section`, `article`, or `cherry`. Each of them would start a full attempt only to fail a few characters later.

When every match must begin with a fixed string, here `class="`, the compiler records that string, and the search compares the whole prefix before starting an attempt:

```diagram
<section class="note">
   ^      candidate 'c' in "section": "ction c" is not "class=\"", skipped
         ^  candidate 'c' in "class": the prefix matches, start an attempt
```

Comparing seven bytes is far cheaper than running the graph.

**Searching eight bytes at a time.** Once most attempts are gone, finding the next candidate byte becomes the main cost. Checking bytes one at a time is slow. A 64-bit processor can instead load eight bytes into one integer and examine all of them at once, using two bit tricks:

1. XOR the eight bytes with the target byte repeated eight times. A byte equal to the target becomes zero; all other bytes stay non-zero.
2. Compute `(x - 0x0101...01) & !x & 0x8080...80`. This well-known expression sets the high bit of every byte that is zero.

Here is the same idea on four bytes, searching for `'c'` (`63`) in `"tcac"`:

```diagram
byte position          0     1     2     3
text                  't'   'c'   'a'   'c'
bytes (hex)            74    63    61    63
step 1: XOR with 63    17    00    02    00
step 2: high bits      00    80    00    80
```

The first marked byte is at position 1, the first `'c'`. If nothing is marked, none of the eight bytes matches, and the search moves on to the next eight. The trick may also mark a byte that follows a match, but never one before the first match, so the first mark is always correct.

Together, the byte table, prefix check, and eight-byte search eliminate most attempts for patterns that begin with something specific.

### 6.3 Skipping Runs That Already Failed

**The problem.** The byte table cannot help with `[a-z]+ing`: in English text, almost every byte is a lowercase letter and therefore a candidate. Worse, the attempts repeat each other's work. Take the word `"jumped"`, which contains no `"ing"`. `[a-z]+` is greedy, so each attempt first takes all the letters and then gives them back one at a time, checking for `"ing"` after each:

```diagram
j u m p e d
0 1 2 3 4 5 6

attempt at 0: [a-z]+ takes "jumped", then checks for "ing" at 6, 5, 4, 3, 2, 1
attempt at 1: [a-z]+ takes "umped",  then checks for "ing" at 6, 5, 4, 3, 2
attempt at 2: [a-z]+ takes "mped",   then checks for "ing" at 6, 5, 4, 3
...
attempt at 5: [a-z]+ takes "d",      then checks for "ing" at 6
```

Look closely: every position checked by the attempt at 1 was already checked by the attempt at 0, and the same holds for every later attempt. They cannot succeed where the first attempt failed.

**The shortcut.** When a pattern begins with an unbounded repetition of a single character class, such as `S+`, `S*`, or `S{n,}`, and an attempt fails, the search jumps past the whole run of `S` characters instead of moving forward by one. In `"she jumped over"`, the search makes three attempts, one per word, instead of thirteen, one per letter:

```diagram
she jumped over
^              attempt at 0 fails, skip to the end of "she"
    ^          attempt at 4 fails, skip to the end of "jumped"
           ^   attempt at 11 fails, skip to the end of "over"
```

**Why it is safe.** After the run, the rest of the pattern sees only where the run ends, never where it began. Starting later inside the run can only end it at positions that were already tried.

**When it is not safe.** Two kinds of patterns break this argument, and the compiler excludes both:

- **Backreferences** make the rest of the pattern depend on where the run began. With `(a+)b\1` and the text `"aaaba"`:

  ```diagram
  a a a b a
  0 1 2 3 4

  attempt at 0: (a+) captures "aaa", then needs "b" and "aaa": fails
                (shorter captures "aa" and "a" are not followed by "b")
  attempt at 1: (a+) captures "aa",  then needs "b" and "aa":  fails
  attempt at 2: (a+) captures "a",   then needs "b" and "a":   matches "aba"
  ```

  Skipping from position 0 to the end of the run would miss the match at position 2. Therefore, any pattern containing a backreference does not use this shortcut.

- **Something before the run**, such as a lookbehind, may fail at the first position but succeed at a later one. With `(?<=x)[a-z]+ing` and the text `"axbing"`, the attempt at 0 fails because no `'x'` precedes it, but the attempt at 2 matches `"bing"`. Therefore, the repetition must be the very first element of the pattern.

### 6.4 Matching a Run of Characters in One Step

**The problem.** The repetition component in section 4.2 is general: its inner component can be anything, including a group with captures or another repetition. That generality has a price. Here is what the component does for each letter that `[a-z]+` consumes:

```diagram
for every letter:
    jump into the inner component
    run the character set transition       <- the only step that does real work
    increment the counter                  (and record the change in the undo log)
    run the repetition back transition     (and save the forward transition as an alternative)
```

That is four transitions, plus stack and undo-log entries, to consume one character. Yet most repetitions in practice repeat a single character: `\d{4}`, `[a-z]+`, `.*`.

**The shortcut.** For a greedy repetition of a single character, the compiler emits one char run transition instead of the whole loop:

```diagram
  /-----------------------------\
  |          char run           |
  |        | transition         |
  |        v                    |
=====o==-------------------==o=====
  | in node            out node |
  |                             |
  \----- char run component ----/
```

The char run consumes matching characters in a simple loop, up to the maximum count. Once it has reached the minimum count, it saves the current position as an alternative ("the run could also stop here") before consuming each further character. It then continues with the longest run. If the rest of the pattern fails, backtracking pops the saved positions one by one, giving back one character at a time, exactly like the general component.

Here is `[a-z]+ing` matching `"running"`:

```diagram
r u n n i n g
0 1 2 3 4 5 6 7

the char run consumes all 7 letters and saves positions 1, 2, 3, 4, 5, 6
continue at 7: "ing" fails (end of text)
pop 6:         "ing" fails ("g")
pop 5:         "ing" fails ("ng")
pop 4:         "ing" matches, the result is (0, 7)
```

For `\d{4}`, the minimum and maximum are both four, so the char run consumes exactly four digits and saves no alternatives, because there is nothing to give back.

**Why it is safe.** The saved positions are tried longest first, the same order in which the general component gives characters back, so both produce exactly the same matches. A single character contains no captures, counters, or nested alternatives, so there is no other state to restore.

**Why lazy repetitions are excluded.** A lazy repetition such as `[a-z]+?` tries the shortest run first and extends it one character at a time. A char run would have to rescan the run from the beginning for every extension, so lazy repetitions keep the general component.

### 6.5 Making Each Step Cheaper

The remaining optimizations do not skip attempts. Instead, they make common steps cheaper:

- **Nodes with one path skip the stack.** A node with several outgoing transitions is a choice point: before trying the first, the processor must save the others for backtracking. Most nodes, however, have only one outgoing transition. In `\d{4}-\d{2}-\d{2}`, for example, the nodes simply lead from one part to the next. Pushing such a transition onto the stack only to pop it again immediately is wasted work, so the processor runs it directly.
- **Strings are compared as bytes.** To match the literal `class="`, the original engine decoded each UTF-8 character of the text and compared it with the expected character. Two UTF-8 strings are equal exactly when their bytes are equal, so a single comparison of seven bytes now suffices.
- **Character sets precompute ASCII membership.** Checking whether a character belongs to `[A-Za-z0-9._%+-]` means testing three ranges and five single characters. The compiler answers this question in advance for all 128 ASCII characters and stores the answers in a table, so checking an ASCII character takes a single lookup. Only non-ASCII characters, such as `'é'`, still check the ranges.
- **Transition stacks are reused.** Every search used to allocate a new stack, grow it while saving alternatives, and free it afterward. When an iterator finds 25,000 matches, that means 25,000 allocations. The context now keeps emptied stacks and hands them out again, much like reusing a shopping bag instead of taking a new one each time.

### 6.6 Measuring the Results

Optimizations must be measured, not guessed: an idea that seems clever may not help at all, and a dull one may help a lot. The [benchmark](../benchmark/src/main.rs) crate runs fifteen patterns over generated English, HTML, and Unicode text in the `examples` directory, and compares this engine with the widely used [`regex`](https://crates.io/crates/regex) crate. Before timing anything, it checks that both engines report the same matches, so the benchmark also guards against optimizations that change results.

```bash
cd benchmark
cargo run --release --bin generate   # create the sample text
cargo run --release                  # run all cases
cargo run --release -- email         # run cases whose names contain "email"
```

The following table shows how much slower this engine is than `regex` after each step, as a geometric mean over all cases:

| Step | Slower than `regex` |
|------|---------------------|
| Original engine | 137x |
| Skipping impossible start positions | 20x |
| Cheaper steps (single-path nodes, byte comparison, ASCII tables) | 16.5x |
| Skipping runs that already failed | 15x |
| Literal prefixes, eight-byte search, and stack reuse | 9.6x |
| Char runs | 5.3x |

Some individual cases, measured on one machine (your numbers will differ, but the proportions should be similar):

| Case | Pattern | Before (ms) | After (ms) | `regex` (ms) |
|------|---------|-------------|------------|--------------|
| literal | `pattern` | 22.3 | 0.30 | 0.06 |
| date | `\d{4}-\d{2}-\d{2}` | 49.7 | 1.35 | 0.07 |
| capital-word | `\b[A-Z][a-z]+\b` | 35.8 | 1.89 | 2.77 |
| html-tag | `<([a-z]+)[^>]*>` | 19.5 | 3.30 | 2.97 |
| cjk-run | `[一-龥]+` | 14.2 | 2.13 | 2.42 |
| alternation | `apple\|banana\|cherry` | 72.9 | 7.24 | 0.30 |
| email | `[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}` | 257.5 | 23.6 | 0.11 |

Together, these optimizations made the engine about 25 times faster. The gap to `regex` still varies widely:

- When matches are short and frequent, as with capitalized words, HTML tags, or runs of Chinese characters, this engine is now about as fast as `regex`, and sometimes faster.
- The gap remains large for alternations of several words, for literal searches, and above all for patterns like email addresses, where the engine starts an attempt at almost every word. In these cases, the `regex` crate relies on techniques beyond the scope of this book, such as SIMD substring search and finite automata, which never backtrack.

In exchange, this engine still compiles patterns faster than `regex`, and its design remains small enough to read and understand from start to finish.

To summarize the lessons of this chapter:

1. **Measure first.** Find out where the time goes before changing anything.
2. **Skip work before speeding it up.** Not starting a hopeless attempt beats running it quickly.
3. **Prove every shortcut safe.** Each optimization must leave the results unchanged, and it must be disabled for the patterns where that proof does not hold.
4. **Test against a reference.** Comparing results with another engine catches mistakes that reasoning alone may miss.
