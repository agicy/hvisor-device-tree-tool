//! This module's main focus is `parse_dt` which parses a DTS file and returns
//! an unflattened device tree.
//!
//! In addition there are a couple of utility C-style escaped strings and
//! characters parsing functions.

#![allow(trivial_numeric_casts)]

use std::num::ParseIntError;
use std::str::{self, FromStr};

use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, tag_no_case, take, take_until, take_while, take_while1};
use nom::character::complete::{
    alpha1, char, digit1, hex_digit1, line_ending, multispace1, not_line_ending, oct_digit1, space1,
};
use nom::combinator::{map, map_res, opt, peek, recognize, rest, value, verify};
use nom::multi::{many0, many1, separated_list1};
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::{IResult, Parser};

use super::ParseError;
use super::tree::{Cell, DTInfo, Data, Node, NodeName, Property, ReserveInfo};

// Copied and modified from rust-lang/rust/src/libcore/num/mod.rs
trait FromStrRadix: PartialOrd + Copy {
    type Err;
    fn from_str_radix(s: &str, radix: u32) -> Result<Self, Self::Err>;
}

macro_rules! doit {
    ($($t:ty)*) => ($(impl FromStrRadix for $t {
        type Err = ParseIntError;
        fn from_str_radix(s: &str, r: u32) -> Result<$t, Self::Err> { Self::from_str_radix(s, r) }
    })*)
}
doit! { i8 i16 i32 i64 isize u8 u16 u32 u64 usize }

fn from_str_hex<T: FromStrRadix>(s: &str) -> Result<T, T::Err> {
    T::from_str_radix(s, 16)
}

fn from_str_oct<T: FromStrRadix>(s: &str) -> Result<T, T::Err> {
    T::from_str_radix(s, 8)
}
fn from_str_dec<T: FromStr>(s: &str) -> Result<T, T::Err> {
    T::from_str(s)
}

// Helper to match any character that is not a line ending
#[allow(dead_code)]
fn not_line_ending_str(input: &[u8]) -> IResult<&[u8], &[u8]> {
    not_line_ending(input)
}

fn eat_junk(input: &[u8]) -> IResult<&[u8], ()> {
    value(
        (),
        many0(alt((
            value((), delimited(tag("/*"), take_until("*/"), tag("*/"))),
            value((), tuple((tag("//"), not_line_ending, opt(line_ending)))),
            value(
                (),
                tuple((
                    tag("#"),
                    opt(preceded(opt(space1), tag("line"))),
                    space1,
                    digit1,
                    opt(tuple((space1, not_line_ending))),
                    opt(line_ending),
                )),
            ),
            value((), multispace1),
        ))),
    )(input)
}

// Wrapper to consume junk before the parser
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], O>
where
    F: Parser<&'a [u8], O, nom::error::Error<&'a [u8]>>,
{
    preceded(eat_junk, inner)
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum OprInfix {
    Multiply,
    Divide,
    Modulus,

    Add,
    Subtract,

    LeftShift,
    RightShift,

    Lesser,
    Greater,
    LesserEqual,
    GreaterEqual,
    Equal,
    NotEqual,

    BitAnd,
    BitXor,
    BitOr,

    And,
    Or,
}

impl OprInfix {
    fn apply(&self, a: u64, b: u64) -> u64 {
        match *self {
            OprInfix::Multiply => a * b,
            OprInfix::Divide => a / b,
            OprInfix::Modulus => a % b,

            OprInfix::Add => a + b,
            OprInfix::Subtract => a - b,

            OprInfix::LeftShift => a << b,
            OprInfix::RightShift => a >> b,

            OprInfix::Lesser => {
                if a < b {
                    1
                } else {
                    0
                }
            }
            OprInfix::Greater => {
                if a > b {
                    1
                } else {
                    0
                }
            }
            OprInfix::LesserEqual => {
                if a <= b {
                    1
                } else {
                    0
                }
            }
            OprInfix::GreaterEqual => {
                if a >= b {
                    1
                } else {
                    0
                }
            }
            OprInfix::Equal => {
                if a == b {
                    1
                } else {
                    0
                }
            }
            OprInfix::NotEqual => {
                if a != b {
                    1
                } else {
                    0
                }
            }

            OprInfix::BitAnd => a & b,
            OprInfix::BitXor => a ^ b,
            OprInfix::BitOr => a | b,

            OprInfix::And => {
                if a != 0 && b != 0 {
                    1
                } else {
                    0
                }
            }
            OprInfix::Or => {
                if a != 0 || b != 0 {
                    1
                } else {
                    0
                }
            }
        }
    }
}

fn opr_infix(input: &[u8]) -> IResult<&[u8], OprInfix> {
    alt((
        value(OprInfix::LeftShift, tag("<<")),
        value(OprInfix::RightShift, tag(">>")),
        value(OprInfix::LesserEqual, tag("<=")),
        value(OprInfix::GreaterEqual, tag(">=")),
        value(OprInfix::Equal, tag("==")),
        value(OprInfix::NotEqual, tag("!=")),
        value(OprInfix::And, tag("&&")),
        value(OprInfix::Or, tag("||")),
        value(OprInfix::Multiply, tag("*")),
        value(OprInfix::Divide, tag("/")),
        value(OprInfix::Modulus, tag("%")),
        value(OprInfix::Add, tag("+")),
        value(OprInfix::Subtract, tag("-")),
        value(OprInfix::Lesser, tag("<")),
        value(OprInfix::Greater, tag(">")),
        value(OprInfix::BitAnd, tag("&")),
        value(OprInfix::BitXor, tag("^")),
        value(OprInfix::BitOr, tag("|")),
    ))(input)
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum OprPrefix {
    Negate,
    BitNot,
    Not,
}

impl OprPrefix {
    fn apply(&self, a: u64) -> u64 {
        match *self {
            OprPrefix::Negate => a.wrapping_neg(),
            OprPrefix::BitNot => a ^ u64::max_value(),
            OprPrefix::Not => {
                if a == 0 {
                    1
                } else {
                    0
                }
            }
        }
    }
}

fn opr_prefix(input: &[u8]) -> IResult<&[u8], OprPrefix> {
    alt((
        value(OprPrefix::Not, tag("!")),
        value(OprPrefix::BitNot, tag("~")),
        value(OprPrefix::Negate, tag("-")),
    ))(input)
}

#[derive(Debug)]
enum Token {
    Number(u64),
    Prefix(OprPrefix),
    Infix(OprInfix),
    Paren,
}

fn parse_c_expr(input: &[u8]) -> IResult<&[u8], u64> {
    let mut stack = Vec::new();
    let mut buf = input;
    loop {
        // Eat junk first
        let (cleaned, _) = eat_junk(buf)?;
        buf = cleaned;

        // Try integer
        if let Ok((rem, num)) = integer(buf) {
            match stack.pop() {
                None => stack.push(Token::Number(num)),
                Some(Token::Paren) => {
                    stack.push(Token::Paren);
                    stack.push(Token::Number(num));
                }
                Some(Token::Prefix(ref x)) => {
                    let num = x.apply(num);
                    stack.push(Token::Number(num));
                }
                Some(Token::Infix(ref x)) => {
                    if let Some(Token::Number(a)) = stack.pop() {
                        let num = x.apply(a, num);
                        stack.push(Token::Number(num));
                    } else {
                        return Err(nom::Err::Error(nom::error::Error::new(
                            buf,
                            nom::error::ErrorKind::Verify,
                        )));
                    }
                }
                Some(Token::Number(a)) if stack.is_empty() => return Ok((buf, a)),
                _ => {
                    return Err(nom::Err::Error(nom::error::Error::new(
                        buf,
                        nom::error::ErrorKind::Verify,
                    )));
                }
            };
            buf = rem;
            continue;
        }

        // Try infix
        if let Ok((rem, tok)) = opr_infix(buf) {
            if tok == OprInfix::Greater {
                let in_paren = stack.iter().any(|t| matches!(t, Token::Paren));
                if !in_paren {
                    if let Some(&Token::Number(a)) = stack.last() {
                        if stack.len() == 1 {
                            return Ok((buf, a));
                        }
                    }
                    return Err(nom::Err::Error(nom::error::Error::new(
                        buf,
                        nom::error::ErrorKind::Verify,
                    )));
                }
            }

            if let Some(&Token::Number(a)) = stack.last() {
                if (tok == OprInfix::Greater || tok == OprInfix::BitAnd) && stack.len() == 1 {
                    // Lookahead check for potential ambiguity
                    let (cleaned_la, _) = eat_junk(rem)?;
                    let is_start_of_term =
                        alt((tag("("), recognize(opr_prefix), recognize(integer)))(cleaned_la)
                            .is_ok();

                    if !is_start_of_term {
                        return Ok((buf, a));
                    }
                }
                stack.push(Token::Infix(tok));
            } else if tok == OprInfix::Subtract {
                stack.push(Token::Prefix(OprPrefix::Negate));
            } else {
                return Err(nom::Err::Error(nom::error::Error::new(
                    buf,
                    nom::error::ErrorKind::Verify,
                )));
            };
            buf = rem;
            continue;
        }

        // Try prefix
        if let Ok((rem, tok)) = opr_prefix(buf) {
            if let Some(&Token::Number(a)) = stack.last() {
                if stack.len() == 1 {
                    return Ok((buf, a));
                }
                return Err(nom::Err::Error(nom::error::Error::new(
                    buf,
                    nom::error::ErrorKind::Verify,
                )));
            } else {
                stack.push(Token::Prefix(tok));
            };
            buf = rem;
            continue;
        }

        // Try special chars
        if let Some(&c) = buf.first() {
            match c {
                b'(' => {
                    match stack.pop() {
                        None => stack.push(Token::Paren),
                        Some(x @ Token::Paren)
                        | Some(x @ Token::Prefix(_))
                        | Some(x @ Token::Infix(_)) => {
                            stack.push(x);
                            stack.push(Token::Paren);
                        }
                        Some(Token::Number(a)) => {
                            if stack.is_empty() {
                                return Ok((buf, a));
                            } else {
                                return Err(nom::Err::Error(nom::error::Error::new(
                                    buf,
                                    nom::error::ErrorKind::Verify,
                                )));
                            }
                        }
                    };
                }
                b')' => {
                    if let Some(Token::Number(num)) = stack.pop() {
                        if let Some(Token::Paren) = stack.pop() {
                            match stack.pop() {
                                None => stack.push(Token::Number(num)),
                                Some(Token::Paren) => {
                                    stack.push(Token::Paren);
                                    stack.push(Token::Number(num));
                                }
                                Some(Token::Prefix(ref x)) => {
                                    let num = x.apply(num);
                                    stack.push(Token::Number(num));
                                }
                                Some(Token::Infix(ref x)) => {
                                    if let Some(Token::Number(a)) = stack.pop() {
                                        let num = x.apply(a, num);
                                        stack.push(Token::Number(num));
                                    } else {
                                        return Err(nom::Err::Error(nom::error::Error::new(
                                            buf,
                                            nom::error::ErrorKind::Verify,
                                        )));
                                    }
                                }
                                _ => {
                                    return Err(nom::Err::Error(nom::error::Error::new(
                                        buf,
                                        nom::error::ErrorKind::Verify,
                                    )));
                                }
                            };
                        } else {
                            return Err(nom::Err::Error(nom::error::Error::new(
                                buf,
                                nom::error::ErrorKind::Verify,
                            )));
                        }
                    } else {
                        return Err(nom::Err::Error(nom::error::Error::new(
                            buf,
                            nom::error::ErrorKind::Verify,
                        )));
                    }
                }
                b';' => {
                    if let Some(Token::Number(a)) = stack.pop() {
                        if stack.is_empty() {
                            return Ok((buf, a));
                        }
                    }
                    return Err(nom::Err::Error(nom::error::Error::new(
                        buf,
                        nom::error::ErrorKind::Verify,
                    )));
                }
                _ => {
                    // Unimplemented char or end of expression
                    if let Some(Token::Number(a)) = stack.pop() {
                        if stack.is_empty() {
                            return Ok((buf, a));
                        }
                    }
                    return Err(nom::Err::Error(nom::error::Error::new(
                        buf,
                        nom::error::ErrorKind::Verify,
                    )));
                }
            }
            buf = &buf[1..];
        } else {
            // Incomplete or empty
            if stack.len() == 1 {
                if let Some(&Token::Number(num)) = stack.last() {
                    return Ok((buf, num));
                }
            }
            return Err(nom::Err::Error(nom::error::Error::new(
                buf,
                nom::error::ErrorKind::Verify,
            )));
        }
    }
}

fn integer(input: &[u8]) -> IResult<&[u8], u64> {
    terminated(
        alt((
            map_res(
                preceded(tag_no_case("0x"), recognize(hex_digit1)),
                |s: &[u8]| from_str_hex(str::from_utf8(s).unwrap()),
            ),
            map_res(
                preceded(tag_no_case("0"), recognize(oct_digit1)),
                |s: &[u8]| from_str_oct(str::from_utf8(s).unwrap()),
            ),
            map_res(recognize(digit1), |s: &[u8]| {
                from_str_dec(str::from_utf8(s).unwrap())
            }),
        )),
        opt(alt((tag("ULL"), tag("LL"), tag("UL"), tag("L"), tag("U")))),
    )(input)
}

fn is_prop_node_char(c: u8) -> bool {
    nom::character::is_alphanumeric(c)
        || c == b','
        || c == b'.'
        || c == b'_'
        || c == b'+'
        || c == b'*'
        || c == b'#'
        || c == b'?'
        || c == b'@'
        || c == b'-'
}

fn is_path_char(c: u8) -> bool {
    is_prop_node_char(c) || c == b'/'
}

fn is_label_char(c: u8) -> bool {
    nom::character::is_alphanumeric(c) || c == b'_'
}

fn parse_label(input: &[u8]) -> IResult<&[u8], String> {
    map(
        map_res(
            recognize(preceded(alt((alpha1, tag("_"))), take_while(is_label_char))),
            str::from_utf8,
        ),
        String::from,
    )(input)
}

fn parse_ref(input: &[u8]) -> IResult<&[u8], String> {
    alt((
        preceded(
            char('&'),
            delimited(
                char('{'),
                map(
                    map_res(take_while1(is_path_char), str::from_utf8),
                    String::from,
                ),
                char('}'),
            ),
        ),
        preceded(
            char('&'),
            map(
                map_res(take_while1(is_label_char), str::from_utf8),
                String::from,
            ),
        ),
    ))(input)
}

fn transform(input: &[u8]) -> IResult<&[u8], Vec<u8>> {
    map(
        many0(alt((
            map(is_not("\\\""), |s: &[u8]| s.to_vec()),
            preceded(
                char('\\'),
                alt((
                    value(vec![b'\x07'], tag("a")),
                    value(vec![b'\x08'], tag("b")),
                    value(vec![b'\t'], tag("t")),
                    value(vec![b'\n'], tag("n")),
                    value(vec![b'\x0B'], tag("v")),
                    value(vec![b'\x0C'], tag("f")),
                    value(vec![b'\r'], tag("r")),
                    value(vec![b'\\'], tag("\\")),
                    value(vec![b'\"'], tag("\"")),
                    map_res(preceded(tag_no_case("x"), hex_digit1), |s: &[u8]| {
                        from_str_hex::<u8>(str::from_utf8(s).unwrap()).map(|b| vec![b])
                    }),
                    map_res(oct_digit1, |s: &[u8]| {
                        from_str_oct::<u8>(str::from_utf8(s).unwrap()).map(|b| vec![b])
                    }),
                )),
            ),
        ))),
        |v| v.concat(),
    )(input)
}

// Custom implementation since escaped_transform requires a specific control flow
pub fn escape_c_string(input: &[u8]) -> IResult<&[u8], String> {
    map_res(
        alt((
            transform,
            map(
                verify(take_until("\""), |s: &[u8]| s.is_empty()),
                |_| vec![],
            ),
        )),
        |v| String::from_utf8(v),
    )(input)
}

pub fn escape_c_char(input: &[u8]) -> IResult<&[u8], u8> {
    alt((
        value(0x07, tag("\\a")),
        value(0x08, tag("\\b")),
        value(b'\t', tag("\\t")),
        value(b'\n', tag("\\n")),
        value(0x0B, tag("\\v")),
        value(0x0C, tag("\\f")),
        value(b'\r', tag("\\r")),
        value(b'\\', tag("\\\\")),
        value(b'\'', tag("\\\'")),
        preceded(
            tag_no_case("\\x"),
            map_res(hex_digit1, |s: &[u8]| {
                from_str_hex::<u8>(str::from_utf8(s).unwrap())
            }),
        ),
        preceded(
            tag("\\"),
            map_res(oct_digit1, |s: &[u8]| {
                from_str_oct::<u8>(str::from_utf8(s).unwrap())
            }),
        ),
        map(take(1usize), |c: &[u8]| c[0]),
    ))(input)
}

fn parse_cell(bits: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Cell> {
    move |input: &[u8]| {
        ws(alt((
            map(
                verify(
                    alt((
                        map(delimited(char('\''), escape_c_char, char('\'')), u64::from),
                        parse_c_expr,
                    )),
                    move |&num| {
                        if bits != 64 {
                            let mask = (1 << bits) - 1;
                            !((num > mask) && ((num | mask) != u64::max_value()))
                        } else {
                            true
                        }
                    },
                ),
                Cell::Num,
            ),
            map(verify(parse_ref, move |_: &String| bits == 32), |s| {
                Cell::Ref(s, None)
            }),
        )))(input)
    }
}

fn parse_mem_reserve(input: &[u8]) -> IResult<&[u8], ReserveInfo> {
    ws(map(
        tuple((
            many0(terminated(parse_label, char(':'))),
            tag("/memreserve/"),
            parse_c_expr,
            parse_c_expr,
            char(';'),
        )),
        |(labels, _, addr, size, _)| ReserveInfo {
            address: addr,
            size: size,
            labels: labels,
        },
    ))(input)
}

fn parse_data_cells(input: &[u8]) -> IResult<&[u8], Data> {
    let (input, bits) = verify(
        map(
            opt(ws(preceded(
                tag("/bits/"),
                map_res(take_until("<"), |s: &[u8]| {
                    from_str_dec::<u64>(str::from_utf8(s).unwrap())
                }),
            ))),
            |b: Option<u64>| b.unwrap_or(32),
        ),
        |&b| b == 8 || b == 16 || b == 32 || b == 64,
    )(input)?;

    let (input, _) = ws(char('<'))(input)?;
    let (input, val) = many0(ws(parse_cell(bits as usize)))(input)?;
    let (input, _) = ws(char('>'))(input)?;

    Ok((input, Data::Cells(bits as usize, val)))
}

enum Item {
    Prop(Property),
    Node(Node),
}

fn parse_contents(
    input_len: usize,
) -> impl FnMut(&[u8]) -> IResult<&[u8], (Vec<Property>, Vec<Node>)> {
    move |input: &[u8]| {
        let (input, items) = many0(parse_item(input_len))(input)?;
        let mut props = Vec::new();
        let mut nodes = Vec::new();
        for item in items {
            match item {
                Item::Prop(p) => props.push(p),
                Item::Node(n) => nodes.push(n),
            }
        }
        Ok((input, (props, nodes)))
    }
}

fn parse_prop_or_node(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Item> {
    move |input: &[u8]| {
        let (input, offset_start) = peek(rest)(input)?;
        let offset = offset_start.len();

        let (input, labels) = many0(terminated(parse_label, char(':')))(input)?;
        let (input, _) = eat_junk(input)?;
        let (input, name) = map(
            map_res(
                alt((take_while1(is_prop_node_char), tag("/"))),
                str::from_utf8,
            ),
            String::from,
        )(input)?;

        let (input, _) = eat_junk(input)?;
        let (input, next_char) = peek(take(1usize))(input)?;

        if next_char == b"{" {
            let (input, _) = char('{')(input)?;
            let (input, (props, children)) = parse_contents(input_len)(input)?;
            let (input, _) = ws(char('}'))(input)?;
            let (input, _) = ws(char(';'))(input)?;

            Ok((
                input,
                Item::Node(Node::Existing {
                    name: NodeName::Full(name),
                    proplist: props
                        .into_iter()
                        .map(|p| (p.name().to_owned(), p))
                        .collect(),
                    children: children
                        .into_iter()
                        .map(|n| (n.name().as_str().to_owned(), n))
                        .collect(),
                    labels,
                    offset: input_len - offset,
                }),
            ))
        } else {
            let (input, val) = opt(preceded(
                ws(char('=')),
                separated_list1(ws(char(',')), parse_data),
            ))(input)?;
            let (input, _) = ws(char(';'))(input)?;

            Ok((
                input,
                Item::Prop(Property::Existing {
                    name,
                    val,
                    labels,
                    offset: input_len - offset,
                }),
            ))
        }
    }
}

fn parse_item(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Item> {
    move |input: &[u8]| {
        ws(alt((
            map(
                tuple((
                    peek(rest),
                    tag("/delete-node/"),
                    map(
                        map_res(take_while1(is_prop_node_char), str::from_utf8),
                        String::from,
                    ),
                    char(';'),
                )),
                |(rem, _, name, _)| {
                    let offset = rem.len();
                    Item::Node(Node::Deleted {
                        name: NodeName::Full(name),
                        offset: input_len - offset,
                    })
                },
            ),
            map(
                tuple((
                    peek(rest),
                    tag("/delete-property/"),
                    map(
                        map_res(take_while1(is_prop_node_char), str::from_utf8),
                        String::from,
                    ),
                    char(';'),
                )),
                |(rem, _, name, _)| {
                    let offset = rem.len();
                    Item::Prop(Property::Deleted {
                        name,
                        offset: input_len - offset,
                    })
                },
            ),
            parse_prop_or_node(input_len),
        )))(input)
    }
}

fn parse_data(input: &[u8]) -> IResult<&[u8], Data> {
    ws(alt((
        delimited(char('"'), map(escape_c_string, Data::String), char('"')),
        parse_data_cells,
        delimited(
            char('['),
            map(
                many1(map_res(
                    map_res(ws(take(2usize)), str::from_utf8),
                    from_str_hex::<u8>,
                )),
                Data::ByteArray,
            ),
            char(']'),
        ),
        map(parse_ref, |x| Data::Reference(x, None)),
    )))(input)
}

#[allow(unused)]
fn parse_prop(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Property> {
    move |input: &[u8]| {
        ws(alt((
            map(
                tuple((
                    peek(rest),
                    tag("/delete-property/"),
                    map(
                        map_res(take_while1(is_prop_node_char), str::from_utf8),
                        String::from,
                    ),
                    char(';'),
                )),
                |(rem, _, name, _)| {
                    let offset = rem.len();
                    Property::Deleted {
                        name,
                        offset: input_len - offset,
                    }
                },
            ),
            map(
                tuple((
                    peek(rest),
                    many0(terminated(parse_label, char(':'))),
                    map(
                        map_res(take_while1(is_prop_node_char), str::from_utf8),
                        String::from,
                    ),
                    opt(preceded(
                        ws(char('=')),
                        separated_list1(ws(char(',')), parse_data),
                    )),
                    ws(char(';')),
                )),
                |(rem, labels, name, data, _)| {
                    let offset = rem.len();
                    Property::Existing {
                        name,
                        val: data,
                        labels,
                        offset: input_len - offset,
                    }
                },
            ),
        )))(input)
    }
}

fn parse_node(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Node> {
    move |input: &[u8]| {
        ws(alt((
            map(
                tuple((
                    peek(rest),
                    tag("/delete-node/"),
                    map(
                        map_res(take_while1(is_prop_node_char), str::from_utf8),
                        String::from,
                    ),
                    char(';'),
                )),
                |(rem, _, name, _)| {
                    let offset = rem.len();
                    Node::Deleted {
                        name: NodeName::Full(name),
                        offset: input_len - offset,
                    }
                },
            ),
            map(
                tuple((
                    peek(rest),
                    many0(terminated(parse_label, char(':'))),
                    map(
                        map_res(
                            alt((take_while1(is_prop_node_char), tag("/"))),
                            str::from_utf8,
                        ),
                        String::from,
                    ),
                    ws(char('{')),
                    parse_contents(input_len),
                    ws(char('}')),
                    ws(char(';')),
                )),
                |(rem, labels, name, _, (props, subnodes), _, _)| {
                    let offset = rem.len();
                    Node::Existing {
                        name: NodeName::Full(name),
                        proplist: props
                            .into_iter()
                            .map(|p| (p.name().to_owned(), p))
                            .collect(),
                        children: subnodes
                            .into_iter()
                            .map(|n| (n.name().as_str().to_owned(), n))
                            .collect(),
                        labels,
                        offset: input_len - offset,
                    }
                },
            ),
        )))(input)
    }
}

fn parse_amend(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Node> {
    move |input: &[u8]| {
        ws(alt((
            map(
                tuple((peek(rest), tag("/delete-node/"), parse_ref, char(';'))),
                |(rem, _, name, _)| {
                    let offset = rem.len();
                    Node::Deleted {
                        name: NodeName::Ref(name),
                        offset: input_len - offset,
                    }
                },
            ),
            map(
                tuple((
                    peek(rest),
                    many0(terminated(parse_label, char(':'))),
                    alt((
                        map(
                            map(map_res(tag("/"), str::from_utf8), String::from),
                            NodeName::Full,
                        ),
                        map(parse_ref, NodeName::Ref),
                    )),
                    ws(char('{')),
                    parse_contents(input_len),
                    ws(char('}')),
                    ws(char(';')),
                )),
                |(rem, labels, name, _, (props, subnodes), _, _)| {
                    let offset = rem.len();
                    Node::Existing {
                        name,
                        proplist: props
                            .into_iter()
                            .map(|p| (p.name().to_owned(), p))
                            .collect(),
                        children: subnodes
                            .into_iter()
                            .map(|n| (n.name().as_str().to_owned(), n))
                            .collect(),
                        labels,
                        offset: input_len - offset,
                    }
                },
            ),
        )))(input)
    }
}

fn parse_device_tree(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Node> {
    move |input: &[u8]| ws(preceded(peek(ws(char('/'))), parse_node(input_len)))(input)
}

fn parse_dts(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], (DTInfo, Vec<Node>)> {
    move |input: &[u8]| {
        ws(map(
            tuple((
                opt(ws(tag("/dts-v1/;"))),
                many0(parse_mem_reserve),
                parse_device_tree(input_len),
                many0(parse_amend(input_len)),
            )),
            |(_, mem_reserves, device_tree, amendments)| {
                (
                    DTInfo {
                        reserve_info: mem_reserves,
                        root: device_tree,
                        boot_cpuid: 0,
                    },
                    amendments,
                )
            },
        ))(input)
    }
}

/// Returned on a successful completion of `parse_dt`.
#[derive(Debug)]
pub enum ParseResult<'a> {
    /// Indicates that the entirety of the buffer was used while parsing. Holds
    /// the boot info that includes the first root node and a `Vec` of all
    /// following nodes.
    Complete(DTInfo, Vec<Node>),
    /// Indicates that only of the buffer was used while parsing. Holds the
    /// device tree info that includes the first root node, a `Vec` of all
    /// following nodes, and a slice containing the remainder of the buffer.
    /// Having left over output after parsing is generally not expected and in
    /// most cases should be considered an error.
    RemainingInput(DTInfo, Vec<Node>, &'a [u8]),
}

/// Parses the slice of `u8`s as ASCII characters and returns a device tree made
/// of the first root node and a `Vec` of nodes defined after that. The nodes
/// defined after the first root node may specify a node by label to modify or
/// my start at the root node. These amendments to the root node can be merged
/// into the device tree manually or by `tree::apply_amends`.
///
/// When a tree and any following nodes are parsed successfully without
/// remaining input `ParseResult::Complete` is returned containing the tree and
/// the following nodes. If there is remaining input
/// `ParseResult::RemainingInput` is returned with the tree, following nodes,
/// and a slice of the remaining input.
///
/// # Errors
/// Returns `ParseError::IncompleteInput` if the end of the input was reached
/// where more was expected.
/// Returns `ParseError::NomError` if a `nom` parsing error was returned. This
/// doesn't help much right now, but will be improved soon.
pub fn parse_dt(source: &[u8]) -> Result<ParseResult<'_>, ParseError> {
    match parse_dts(source.len())(source) {
        Ok((remaining, (tree, amends))) => {
            if remaining.is_empty() {
                Ok(ParseResult::Complete(tree, amends))
            } else {
                Ok(ParseResult::RemainingInput(tree, amends, remaining))
            }
        }
        Err(nom::Err::Incomplete(_)) => Err(ParseError::IncompleteInput),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            eprintln!(
                "Nom Error: {:?}, Input: {:?}",
                e.code,
                std::str::from_utf8(e.input).unwrap_or("not utf8")
            );
            Err(ParseError::NomError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_prop(input_len: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Property> {
        move |input: &[u8]| match parse_item(input_len)(input) {
            Ok((rem, Item::Prop(p))) => Ok((rem, p)),
            Ok((_, Item::Node(_))) => Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            ))),
            Err(e) => Err(e),
        }
    }

    #[test]
    fn prop_empty() {
        let input = b"empty_prop;";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "empty_prop".to_owned(),
                    val: None,
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn prop_cells() {
        let input = b"cell_prop = < 1 2 10 >;";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "cell_prop".to_owned(),
                    val: Some(vec![Data::Cells(
                        32,
                        vec![Cell::Num(1), Cell::Num(2), Cell::Num(10)]
                    )]),
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn prop_strings() {
        let input = b"string_prop = \"string\", \"string2\";";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "string_prop".to_owned(),
                    val: Some(vec![
                        Data::String("string".to_owned()),
                        Data::String("string2".to_owned())
                    ]),
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn prop_bytes() {
        let input = b"bytes_prop = [1234 56 78];";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "bytes_prop".to_owned(),
                    val: Some(vec![Data::ByteArray(vec![0x12, 0x34, 0x56, 0x78])]),
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn prop_mixed() {
        let input = b"mixed_prop = \"abc\", [1234], <0xa 0xb 0xc>;";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "mixed_prop".to_owned(),
                    val: Some(vec![
                        Data::String("abc".to_owned()),
                        Data::ByteArray(vec![0x12, 0x34]),
                        Data::Cells(32, vec![Cell::Num(0xa), Cell::Num(0xb), Cell::Num(0xc)])
                    ]),
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn block_comment() {
        let input = b"test_prop /**/ = < 1 2 10 >;";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "test_prop".to_owned(),
                    val: Some(vec![Data::Cells(
                        32,
                        vec![Cell::Num(1), Cell::Num(2), Cell::Num(10)]
                    )]),
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn line_comment() {
        let input = b"test_prop // stuff\n\t= < 1 2 10 >;";
        assert_eq!(
            parse_prop(input.len())(input),
            Ok((
                &b""[..],
                Property::Existing {
                    name: "test_prop".to_owned(),
                    val: Some(vec![Data::Cells(
                        32,
                        vec![Cell::Num(1), Cell::Num(2), Cell::Num(10)]
                    )]),
                    labels: Vec::new(),
                    offset: 0,
                }
            ))
        );
    }

    #[test]
    fn data_string_pain() {
        assert_eq!(
            parse_data(b"\"\\x7f\\0stuffstuff\\t\\t\\t\\n\\n\\n\""),
            Ok((
                &b""[..],
                Data::String("\x7f\0stuffstuff\t\t\t\n\n\n".to_owned())
            ))
        );
    }

    #[test]
    fn data_string_empty() {
        assert_eq!(
            parse_data(b"\"\""),
            Ok((&b""[..], Data::String("".to_owned())))
        );
    }

    #[test]
    fn data_cell_sized_8_escapes() {
        assert_eq!(
            parse_data(b"/bits/ 8 <'\\r' 'b' '\\0' '\\'' '\\xff' 0xde>"),
            Ok((
                &b""[..],
                Data::Cells(
                    8,
                    vec![
                        Cell::Num(b'\r' as u64),
                        Cell::Num(b'b' as u64),
                        Cell::Num(0),
                        Cell::Num(b'\'' as u64),
                        Cell::Num(0xFF),
                        Cell::Num(0xDE)
                    ]
                )
            ))
        );
    }

    #[test]
    fn data_cell_sized_16_escapes() {
        assert_eq!(
            parse_data(b"/bits/ 16 <'\\r' 'b' '\\0' '\\'' '\\xff' 0xdead>"),
            Ok((
                &b""[..],
                Data::Cells(
                    16,
                    vec![
                        Cell::Num(b'\r' as u64),
                        Cell::Num(b'b' as u64),
                        Cell::Num(0),
                        Cell::Num(b'\'' as u64),
                        Cell::Num(0xFF),
                        Cell::Num(0xDEAD)
                    ]
                )
            ))
        );
    }
}
