## 5. How Regular Expression Engines Work

TODO


In the general impression of developers, regular expressions are used for validating, searching strings. The regular expression text is somewhat like random characters which are typed by a cat rolling on the keyboard. You may perfer searching regular expressions on the internet, and then copy and paste the myth string into your code. Sometimes these expressions do not work, sometimes they work, but you do not know why.

Regular expressions are hard to master because one is that they are designed concisely and compactly, and the other, the more important one, is that few people tell you how they work, they just tell you the how to use them, it is similar to the teacher only telling you the syntax of C programming language, but not telling you how the program runs in the computer.

In this tutorial, I will explain the principal of regular expression from the engine's view. In detal, I will translate the regular expressions into literals and functions, and showing how they work together. At last, you will find that the regular expression is just a simple language which is combination of literals and functions. In the following sections, I will use the term "ANRE" to refer to this simple language, and use the term "regex" to refer to the traditional regular expression.

### 3.1 What exactly do regular expressions do?

In short, regular expressions are used to match and capture characters (yes, it's not about string, but about characters).

The process is a bit like a robot checking each character in a string one by one, and if the robot finds a character is what it is looking for, it will pick it up and put it in a bag. the robot takes the "wishlist" and keep checking the next character it needed until it finds all the characters on the wishlist.

![Detective Duck](docs/images/detective.png)

In the programming world, the "wishlist" is called a "regular expression". The robot is the regular expression engine, and the bag is the memory space where the matched characters are stored. Of course, the engine does not necessarily store the matched characters, but it just store the start and end position of the matched characters for efficiency.

### 3.2 The simpliest regular expression - single character

The simplest regular expression is just a character. For example, the regex `a` will match the character 'a' in a string. The engine will check each character in the string one by one, and if it finds a character that is 'a', it will store the position and end the process.

![Single Character](docs/images/single-char.png)

Some regular expression engines will also provide functions like `find_all` or `match_all` to find all occurrences of the character in the string. The principle is quite simple: the engine just repeats the process of match-and-capture from the position of the last matched.

![Match All](docs/images/match-all.png)

### 3.3 Strings

Matching single characters is less useful in real-world applications. In most cases, we need to match strings. For example, the regex `abc` will match the string "abc" in a larger string.

In the engine, strings are treated as a sequence of characters. The engine will check each character in the string one by one, if all characters are found the engine will store the start and end position and end the process.

It worth meantion that the engine will discard the matched characters if it finds the next character is not what it is currently looking for. This figure illustrates the engine discarding the matched "ab" when it finds the next character is not 'c'.

![Match String](docs/images/match-string.png)

Another important thing is: which position the engine should start in the coming process? The engine will start from the position next to the last start position instead of the last end position. In the above example, the engine will start from the position of 'b', which is next to the last start position (i.e. the position of 'a'), instead of position of 'd' or 'e'. This is similar to the simplest String-searching algorithm - the [naive string search](https://en.wikipedia.org/wiki/String-searching_algorithm#Naive_string_search).

![Match String Success](docs/images/match-string-success.png)

Single characters and strings are the simplest regular expressions, in ANRE they are called _Character literals_ and _String literals_. While there is no "String" type in regex, ANRE distinguishes between character literals which surrounded by single quotes and string literals which are surrounded by double quotes.

| Literal Type | Regex | ANRE | Description |
|--------------|-------|------|-------------|
| Char | `a` | `'a'` | Match a single character |
| String | `abc` or `(abc)` | `"abc"` | Match a series of characters in order |

### 3.4 Route Map

We are using a "wishlist" to represent the regular expression in previous examples, which is sufficient for the simple cases. However, the wishlist is not enough for more complex cases, such as the regular expression contained repetition and branches.

It is time for us to upgrade the representation. You might have notice the process of the match-and-capture is a little bit like a trip in some game, where we start from a certain place, and go through a series of checkpoints (each of which have different requirements), and finally arrive at the destination and the trip is complete. We can using a "route map" which consists of a series of nodes (checkpoints) and edges (the path between the nodes) to represent the regular expression, note that every map has a start node and an end node.

This figure illustrates the route maps of regex `a` and `abc`:

![Route Map](docs/images/route-map.png)

Now we can express how a basic engine works using "game rules":

1. There are two cursors, one is the position in the route map which represents the current requirement, and the other is the position in the string which represents the current character. Set both cursors to 0 at beginning.

2. If the current character matches the current requirement, we move both cursors to the next position.

3. If the current character does not match the current requirement, we reset the route map cursor and move the string cursor to the next position of the last start position.

4. If the route map cursor reaches the end node, we have found a match, we store the start and end position of string and end the game with success.

5. If the string cursor reaches the end of the string, we have not found a match, we end the game with failure.

### 3.5 Charset

Let's introduce another literal type - charset. A charset is a set of characters that can be matched. For example, the regex `[abc]` will match any character that is 'a', 'b', or 'c'. For continous characters, we can use the `-` operator to specify a range of characters. For example, the regex `[0-9]` will match any digit from '0' to '9'.

> Regex `[9-0]` is not valid, because the range must follows the order of the characters in the ASCII table (or unicode code points that we will discuss later).

A charset can contains multiple characters and ranges. For example, the regular expression `[a-zA-Z_]` will match any letter (lowercase or uppercase) or underscore.

![Charset](docs/images/charset.png)

## 6. Implementing Regular Expression Engine

TODO::

