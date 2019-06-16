// These values are returned by the state transition fntions
// assigned to scanner.state and the method scanner.eof.
// They give details about the current state of the scan that
// callers might be interested to know about.
// It is okay to ignore the return value of any particular
// call to scanner.state: if one call returns scanError,
// every subsequent call will return scanError too.

// parsing array value
// A scanner is a JSON scanning state machine.
// Callers call scan.reset() and then pass bytes in one at a time
// by calling scan.step(&scan, c) for each byte.
// The return value, referred to as an opcode, tells the
// caller about significant parsing events like beginning
// and ending literals, objects, and arrays, so that the
// caller can follow along if it wishes.
// The return value scanEnd indicates that a single top-level
// JSON value has been completed, *before* the byte that
// just got passed in.  (The indication must be delayed in order
// to recognize the end of numbers: is 123 a whole value or
// the beginning of 12345e+6?).

// Continue.
const scanContinue: u8 = 0;
// uninteresting byte
const scanBeginLiteral: u8 = 1;
// end implied by next result != scanContinue
const scanBeginObject: u8 = 2;
// begin object
const scanObjectKey: u8 = 3;
// just finished object key (string)
const scanObjectValue: u8 = 4;
// just finished non-last object value
const scanEndObject: u8 = 5;
// end object (implies scanObjectValue if possible)
const scanBeginArray: u8 = 6;
// begin array
const scanArrayValue: u8 = 7;
// just finished array value
const scanEndArray: u8 = 8;
// end array (implies scanArrayValue if possible)
const scanSkipSpace: u8 = 9;
// space byte; can skip; known to be last "continue" result
// Stop.
const scanEnd: u8 = 10;
// top-level value ended *before* this byte; known to be first "stop" result
const scanError: u8 = 11;       // hit an error, scanner.err.


// These values are stored in the parseState stack.
// They give the current state of a composite value
// being scanned. If the parser is inside a nested value
// the parseState describes the nested state, outermost at entry 0.

const parseObjectKey: u8 = 0;
// parsing object key (before colon)
const parseObjectValue: u8 = 1;
// parsing object value (after colon)
const parseArrayValue: u8 = 2;

struct Scanner {
    // The step is a fn to be called to execute the next transition.
    step: fn(s: &mut Scanner, c: char) -> u8,
    // Reached end of top-level value.
    end_top: bool,
    // Stack of what we're in the middle of - array values, object keys, object values.
    parse_state: Vec<u8>,
    error: Option<String>,
    // total bytes consumed, updated by decoder.Decode
    bytes: i64,
}

impl Scanner {
    // reset prepares the scanner for use.
    // It must be called before calling s.step.
    fn reset(&mut self) {
        self.step = stateBeginValue;
        self.end_top = false;
        self.parse_state = vec![];
        self.error = None;
        self.bytes = 0;
    }
    // eof tells the scanner that the end of input has been reached.
    // It returns a scan status just as s.step does.
    fn eof(&mut self) -> u8 {
        if self.error.is_some() {
            return scanError;
        }
        if self.end_top {
            return scanEnd;
        }
        (self.step)(self, ' ');
        if self.end_top {
            return scanEnd;
        }

        if self.error.is_none() {

            let err:String ="unexpected end of JSON input".parse().unwrap();
            // s.err = &SyntaxError { "unexpected end of JSON input", s.bytes
            self.error = Some(err);
        }
        return scanError;
    }


    // pushParseState pushes a new parse state p onto the parse stack.
    fn pushParseState(&mut self, p: u8) {
        self.parse_state.push(p);
    }

    // popParseState pops a parse state (already obtained) off the stack
// and updates s.step accordingly.
    fn popParseState(&mut self) {
        let n: usize = self.parse_state.len() - 1;
        let _ = self.parse_state.pop();
        if n == 0 {
            self.step = state_end_top;
            self.end_top = true
        } else {
            self.step = state_end_value
        }
    }


    // error records an error and switches to the error state.
    fn error(&mut self, c: char, context: &str) -> u8 {
        self.step = stateError;
        self.error = Option::from("invalid character ".to_string() + c.to_string().as_ref() + " " + context);//todo pass bytes
        return scanError;
    }
}


fn isSpace(c: char) -> bool {
    return c == ' ' || c == '\t' || c == '\r' || c == '\n';
}


// stateBeginValueOrEmpty is the state after reading `[`.
fn stateBeginValueOrEmpty(s: &mut Scanner, c: char) -> u8 {
    if c <= ' ' && isSpace(c) {
        return scanSkipSpace;
    }
    if c == ']' {
        return state_end_value(s, c);
    }
    return stateBeginValue(s, c);
}


// stateBeginValue is the state at the beginning of the input.
fn stateBeginValue(s: &mut Scanner, c: char) -> u8 {
    if c <= ' ' && isSpace(c) {
        return scanSkipSpace;
    }

    match c {
        '{' => {
            s.step = stateBeginStringOrEmpty;
            s.pushParseState(parseObjectKey);
            return scanBeginObject;
        }
        '[' => {
            s.step = stateBeginValueOrEmpty;
            s.pushParseState(parseArrayValue);
            return scanBeginArray;
        }

        '"' => {
            s.step = state_in_string;
            return scanBeginLiteral;
        }
        '-' => {
            s.step = state_neg;
            return scanBeginLiteral;
        }
        '0' => { // beginning of 0.123
            s.step = state0;
            return scanBeginLiteral;
        }
        't' => {// beginning of true
            s.step = stateT;
            return scanBeginLiteral;
        }
        'f' => { // beginning of false
            s.step = stateF;
            return scanBeginLiteral;
        }
        'n' => { // beginning of null
            s.step = stateN;
            return scanBeginLiteral;
        }
        _ => {
            if '1' <= c && c <= '9' { // beginning of 1234.5
                s.step = state1;
                return scanBeginLiteral;
            }
            return s.error(c, "looking for beginning of value");
        }//all the other cases
    }
}


// stateBeginStringOrEmpty is the state after reading `{`.
fn stateBeginStringOrEmpty(s: &mut Scanner, c: char) -> u8 {
    if c <= ' ' && isSpace(c) {
        return scanSkipSpace;
    }
    if c == '}' {
        let n: usize = s.parse_state.len();
        s.parse_state[n - 1] = parseObjectValue;
        return state_end_value(s, c);
    }
    return stateBeginString(s, c);
}


// stateBeginString is the state after reading `{"key": value,`.
fn stateBeginString(s: &mut Scanner, c: char) -> u8 {
    if c <= ' ' && isSpace(c) {
        return scanSkipSpace;
    }
    if c == '"' {
        s.step = state_in_string;
        return scanBeginLiteral;
    }
    return s.error(c, "looking for beginning of object key string");
}


// state_end_value is the state after completing a value,
// such as after reading `{}` or `true` or `["x"`.
fn state_end_value(s: &mut Scanner, c: char) -> u8 {
    let n: usize = s.parse_state.len();
    if n == 0 {
// Completed top-level before the current byte.
        s.step = state_end_top;
        s.end_top = true;
        return state_end_top(s, c);
    }
    if c <= ' ' && isSpace(c) {
        s.step = state_end_value;
        return scanSkipSpace;
    }
    let i: usize = s.parse_state.len() - 1;
    let ps: u8 = s.parse_state[i];

    match ps {
        parseObjectKey => {
            if c == ':' {
                s.parse_state[n - 1] = parseObjectValue;
                s.step = stateBeginValue;
                return scanObjectKey;
            }
            return s.error(c, "after object key");
        }
        parseObjectValue => {
            if c == ',' {
                s.parse_state[n - 1] = parseObjectKey;
                s.step = stateBeginString;
                return scanObjectValue;
            }
            if c == '}' {
                s.popParseState();
                return scanEndObject;
            }
            return s.error(c, "after object key:value pair");
        }
        parseArrayValue => {
            if c == ',' {
                s.step = stateBeginValue;
                return scanArrayValue;
            }
            if c == ']' {
                s.popParseState();
                return scanEndArray;
            }
            return s.error(c, "after array element");
        }
        _ => { return s.error(c, ""); }
    }
}

// state_end_top is the state after finishing the top-level value,
// such as after reading `{}` or `[1,2,3]`.
// Only space characters should be seen now.
fn state_end_top(s: &mut Scanner, c: char) -> u8 {
    if !isSpace(c) {
// Complain about non-space byte on next call.
        s.error(c, "after top-level value");
    }
    return scanEnd;
}

// state_in_string is the state after reading `"`.
fn state_in_string(s: &mut Scanner, c: char) -> u8 {
    if c == '"' {
        s.step = state_end_value;
        return scanContinue;
    }
    if c == '\\' {
        s.step = state_in_string_esc;
        return scanContinue;
    }
    if c < 0x20 as char {
        return s.error(c, "in string literal");
    }
    return scanContinue;
}


// state_in_string_esc is the state after reading `"\` during a quoted string.
fn state_in_string_esc(s: &mut Scanner, c: char) -> u8 {
    match c {
        'b' | 'f' | 'n' | 'r' | 't' | '\\' | '/' | '"' => {
            s.step = state_in_string;
            return scanContinue;
        }
        'u' => {
            s.step = state_in_string_esc_u;
            return scanContinue;
        }
        _ => {
            return s.error(c, "in string escape code");
        }
    }
}

// state_in_string_esc_u is the state after reading `"\u` during a quoted string.
fn state_in_string_esc_u(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' || 'a' <= c && c <= 'f' || 'A' <= c && c <= 'F' {
        s.step = state_in_string_esc_u1;
        return scanContinue;
    }
// numbers
    return s.error(c, "in \\u hexadecimal character escape");
}

// state_in_string_esc_u1 is the state after reading `"\u1` during a quoted string.
fn state_in_string_esc_u1(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' || 'a' <= c && c <= 'f' || 'A' <= c && c <= 'F' {
        s.step = state_in_string_esc_u12;
        return scanContinue;
    }
// numbers
    return s.error(c, "in \\u hexadecimal character escape");
}


// state_in_string_esc_u12 is the state after reading `"\u12` during a quoted string.
fn state_in_string_esc_u12(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' || 'a' <= c && c <= 'f' || 'A' <= c && c <= 'F' {
        s.step = state_in_string_esc_u123;
        return scanContinue;
    }
// numbers
    return s.error(c, "in \\u hexadecimal character escape");
}

// state_in_string_esc_u123 is the state after reading `"\u123` during a quoted string.
fn state_in_string_esc_u123(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' || 'a' <= c && c <= 'f' || 'A' <= c && c <= 'F' {
        s.step = state_in_string;
        return scanContinue;
    }
// numbers
    return s.error(c, "in \\u hexadecimal character escape");
}

// state_neg is the state after reading `-` during a number.
fn state_neg(s: &mut Scanner, c: char) -> u8 {
    if c == '0' {
        s.step = state0;
        return scanContinue;
    }
    if '1' <= c && c <= '9' {
        s.step = state1;
        return scanContinue;
    }
    return s.error(c, "in numeric literal");
}


// state1 is the state after reading a non-zero integer during a number,
// such as after reading `1` or `100` but not `0`.
fn state1(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' {
        s.step = state1;
        return scanContinue;
    }
    return state0(s, c);
}

// state0 is the state after reading `0` during a number.
fn state0(s: &mut Scanner, c: char) -> u8 {
    if c == '.' {
        s.step = state_dot;
        return scanContinue;
    }
    if c == 'e' || c == 'E' {
        s.step = state_e;
        return scanContinue;
    }
    return state_end_value(s, c);
}

// state_dot is the state after reading the integer and decimal point in a number,
// such as after reading `1.`.
fn state_dot(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' {
        s.step = state_dot0;
        return scanContinue;
    }
    return s.error(c, "after decimal point in numeric literal");
}

// state_dot0 is the state after reading the integer, decimal point, and subsequent
// digits of a number, such as after reading `3.14`.
fn state_dot0(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' {
        return scanContinue;
    }
    if c == 'e' || c == 'E' {
        s.step = state_e;
        return scanContinue;
    }
    return state_end_value(s, c);
}

// state_e is the state after reading the mantissa and e in a number,
// such as after reading `314e` or `0.314e`.
fn state_e(s: &mut Scanner, c: char) -> u8 {
    if c == '+' || c == '-' {
        s.step = state_esign;
        return scanContinue;
    }
    return state_esign(s, c);
}

// state_esign is the state after reading the mantissa, e, and sign in a number,
// such as after reading `314e-` or `0.314e+`.
fn state_esign(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' {
        s.step = state_e0;
        return scanContinue;
    }
    return s.error(c, "in exponent of numeric literal");
}

// state_e0 is the state after reading the mantissa, e, optional sign,
// and at least one digit of the exponent in a number,
// such as after reading `314e-2` or `0.314e+1` or `3.14e0`.
fn state_e0(s: &mut Scanner, c: char) -> u8 {
    if '0' <= c && c <= '9' {
        return scanContinue;
    }
    return state_end_value(s, c);
}

// stateT is the state after reading `t`.
fn stateT(s: &mut Scanner, c: char) -> u8 {
    if c == 'r' {
        s.step = stateTr;
        return scanContinue;
    }
    return s.error(c, "in literal true (expecting 'r')");
}

// stateTr is the state after reading `tr`.
fn stateTr(s: &mut Scanner, c: char) -> u8 {
    if c == 'u' {
        s.step = stateTru;
        return scanContinue;
    }
    return s.error(c, "in literal true (expecting 'u')");
}

// stateTru is the state after reading `tru`.
fn stateTru(s: &mut Scanner, c: char) -> u8 {
    if c == 'e' {
        s.step = state_end_value;
        return scanContinue;
    }
    return s.error(c, "in literal true (expecting 'e')");
}

// stateF is the state after reading `f`.
fn stateF(s: &mut Scanner, c: char) -> u8 {
    if c == 'a' {
        s.step = stateFa;
        return scanContinue;
    }
    return s.error(c, "in literal false (expecting 'a')");
}

// stateFa is the state after reading `fa`.
fn stateFa(s: &mut Scanner, c: char) -> u8 {
    if c == 'l' {
        s.step = stateFal;
        return scanContinue;
    }
    return s.error(c, "in literal false (expecting 'l')");
}

// stateFal is the state after reading `fal`.
fn stateFal(s: &mut Scanner, c: char) -> u8 {
    if c == 's' {
        s.step = stateFals;
        return scanContinue;
    }
    return s.error(c, "in literal false (expecting 's')");
}

// stateFals is the state after reading `fals`.
fn stateFals(s: &mut Scanner, c: char) -> u8 {
    if c == 'e' {
        s.step = state_end_value;
        return scanContinue;
    }
    return s.error(c, "in literal false (expecting 'e')");
}

// stateN is the state after reading `n`.
fn stateN(s: &mut Scanner, c: char) -> u8 {
    if c == 'u' {
        s.step = stateNu;
        return scanContinue;
    }
    return s.error(c, "in literal null (expecting 'u')");
}

// stateNu is the state after reading `nu`.
fn stateNu(s: &mut Scanner, c: char) -> u8 {
    if c == 'l' {
        s.step = stateNul;
        return scanContinue;
    }
    return s.error(c, "in literal null (expecting 'l')");
}

// stateNul is the state after reading `nul`.
fn stateNul(s: &mut Scanner, c: char) -> u8 {
    if c == 'l' {
        s.step = state_end_value;
        return scanContinue;
    }
    return s.error(c, "in literal null (expecting 'l')");
}


// stateError is the state after reaching a syntax error,
// such as after reading `[1}` or `5.1.2`.
fn stateError(s: &mut Scanner, c: char) -> u8 {
    return scanError;
}


