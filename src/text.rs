use std::{cmp::{min, max}, io::{Stdout, Write}, fs};

use crossterm::{event::KeyCode, style::{Color, SetForegroundColor, self}, queue};

#[derive(Clone)]
pub struct Cursor{
    x: u16,
    y: u16,
    render_x: u16,
    x_offset: u16,
    y_offset: u16,
    size: (u16, u16),
}

impl Cursor {
    pub fn new(size: (u16, u16)) -> Self {
        Self{x: 0, y: 0, render_x:0, x_offset: 0, y_offset: 0, size: size}
    }

    pub fn move_cursor(&mut self, text: &Text, direction: KeyCode) {
        match direction {
            KeyCode::Up => {
                if self.y > 0 {
                    self.y -= 1;
                    self.render_x = min(self.x, text.line_len(self.y as usize) as u16);
                }
            }
            KeyCode::Right => {
                self.x = self.render_x;
                if self.x < text.line_len(self.y as usize) as u16 {
                    self.x += 1;
                }else if self.y < text.len() as u16 - 1 {
                    self.y += 1;
                    self.x = 0;
                }
                self.render_x = self.x;
            }
            KeyCode::Down => {
                if self.y < text.len() as u16 - 1 {
                    self.y += 1;
                    self.render_x = min(self.x, text.line_len(self.y as usize) as u16)
                }
            }
            KeyCode::Left => {
                self.x = self.render_x;
                if self.x > 0 {
                    self.x -= 1;
                }else if self.y > 0 {
                    self.y -= 1;
                    self.x = text.line_len(self.y as usize) as u16;
                }
                self.render_x = self.x;
            }
            _ => {}
        }
    }

    pub fn change_offset(&mut self) {
        if self.y < self.y_offset {   // Up
            self.y_offset -= self.y_offset - self.y;
        }
        if self.render_x> self.size.0 + self.x_offset - 1 {  // Right
            self.x_offset += self.render_x- (self.size.0 + self.x_offset - 1);
        }
        if self.y > self.size.1 + self.y_offset - 1 { // Down
            self.y_offset += self.y - (self.size.1 + self.y_offset - 1);
        }
        if self.render_x< self.x_offset {   // Left
            self.x_offset -= self.x_offset - self.render_x;
        }
    }

    pub fn get_position(&self) -> (u16, u16) {
        (self.render_x, self.y)
    }

    pub fn set_position(&mut self, x: u16, y: u16) {
        self.x = x;
        self.render_x = x;
        self.y = y;
    }

    pub fn get_offset(&self) -> (u16, u16) {
        (self.x_offset, self.y_offset)
    }

    pub fn get_line_index(&self) -> usize {
        self.y as usize
    }
}

pub struct SearchData {
    results: Vec<(u16,u16)>,
    index: usize,
}

impl SearchData {
    pub fn new() -> Self {
        Self{results: Vec::new(), index:0 }
    }

    pub fn find_results(&mut self, phrase: &String, text: &mut Text) -> Option<(u16, u16)> {
        text.update_syntax();

        self.results.clear();
        if phrase.len() == 0 { return None; }
        for row in 0..text.len() {
            let mut start = 0;
            while let Some(result) = text.find_phrase(phrase, row, start) {
                let col = start + result;
                self.results.push((col as u16, row as u16));
                start = col + phrase.len();
            }
        }

        for (x,y) in &self.results {
            for i in 0..phrase.len() {
                text.lines[*y as usize].highlight_types[i + (*x as usize)] = HighlightType::SearchResult;
            }
        }

        if self.results.len() > 0 {
            Some(self.results[0])
        } else {
            None
        }
    }

    pub fn get_next(&mut self) -> Option<(u16, u16)> {
        if self.results.len() == 0 {
            None
        }else{
            self.index = (self.index + 1) % self.results.len();
            Some(self.results[self.index])
        }
    }

    pub fn get_previous(&mut self) -> Option<(u16, u16)> {
        if self.results.len() == 0 {
            None
        }else{
            self.index = (self.index + self.results.len() - 1) % self.results.len();
            Some(self.results[self.index])
        }
    }
}

#[derive(Clone, Copy)]
pub enum HighlightType {
    Standard,
    Identity,
    Keyword,
    Number, 
    Bracket,
    String,
    Comment,
    SearchResult,
}

trait SyntaxHighlight {
    fn update_syntax(&self, lines: &mut Vec<Line>);
    fn syntax_colour(&self, highlight_type: &HighlightType) -> Color;

    fn word_len(&self, chars: &[char]) -> usize {
        let mut len = 0;
        while len < chars.len() {
            let c = chars[len];
            if c.is_alphabetic() || c == '_' || (c.is_numeric() && len > 0) {
                len += 1;
            }else{
                break;
            }
        }
        len
    }

    fn number_len(&self, chars: &[char]) -> usize {
        let mut len = 0;
        while len < chars.len() {
            let c = chars[len];
            if c.is_numeric() || (len > 0 && ['_','.'].contains(&c)) {
                len += 1;
            }else{
                break;
            }
        }
        len
    }

    fn string_len(&self, chars: &[char]) -> usize {
        let start = chars[0];
        if !['"','\''].contains(&start) {
            return 0;
        }
        let mut len = 1;
        let mut is_escaped = false;
        while len < chars.len() {
            let c = chars[len];
            len += 1;
            if !is_escaped && c == start {
                break;
            }
            is_escaped = c == '\\' && !is_escaped;
        }
        len
    }

    fn single_line_comment_len(&self, chars: &[char], start: &str) -> usize{
        if !self.match_sequence(&chars[0..], start) {
            return 0;
        }
        let mut len = start.len();

        while len < chars.len() {
            let c = chars[len];
            len += 1;
            if c == '\n' {
                break;
            }
        }
        len
    }

    fn multi_line_comment_len(&self, chars: &[char], start: &str, end: &str) -> usize{
        if !self.match_sequence(chars, start) {
            return 0;
        }
        let mut len = start.len();
        let mut depth = 1;
        while len < chars.len() {
            if self.match_sequence(&chars[len..], start) {
                depth += 1;
                len += start.len();
                continue;
            }

            if self.match_sequence(&chars[len..], end) {
                depth -= 1;
                len += end.len();       
                continue;
            }


            if depth == 0 {
                break;
            }         

            len += 1;
        }
        len
    }

    fn is_bracket(&self, c: &char) -> bool {
        ['(',')','{','}','[',']'].contains(c)
    }

    fn match_sequence(&self, chars: &[char], sequence: &str) -> bool{
        if chars.len() < sequence.len() {
            return false;
        }
        let mut i = 0;
        for c in sequence.chars() {
            if c != chars[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}

pub struct RustSyntax {
}

impl SyntaxHighlight for RustSyntax {
    fn update_syntax(&self, lines: &mut Vec<Line>) {
        // Find colours
        let mut chars = Vec::new();
        for line in &mut *lines {
            chars.append(&mut line.content.chars().clone().collect());
            chars.push('\n');
        };
        let mut highlight_types = Vec::with_capacity(chars.len());
        let mut i = 0;
        while i < chars.len() {
            let word_len = self.word_len(&chars[i..]);
            if word_len > 0 {
                let keywords = ["impl","fn","pub","struct","enum","trait","use","for","if","while","else","break","return","continue","mod","macro_rules","true","false","loop","match","let","as","mut"];
                let word: String = chars[i..i+word_len].iter().collect();
                let highlight_type = if keywords.contains(&word.as_str()) {
                    HighlightType::Keyword
                } else {
                    HighlightType::Identity
                };
                highlight_types.append(&mut vec![highlight_type; word_len]);
                i += word_len;
                continue;
            }

            let number_len = self.number_len(&chars[i..]);
            if number_len > 0 {
                highlight_types.append(&mut vec![HighlightType::Number; number_len]);
                i += number_len;
                continue;
            }

            let string_len = self.string_len(&chars[i..]);
            if string_len > 0 {
                highlight_types.append(&mut vec![HighlightType::String; string_len]);
                i += string_len;
                continue;
            }

            if self.is_bracket(&chars[i]){
                highlight_types.push(HighlightType::Bracket);
                i += 1;
                continue;
            }

            let comment_len = max(
                self.single_line_comment_len(&chars[i..], "//"),
                self.multi_line_comment_len(&chars[i..], "/*", "*/"), 
            );
            if comment_len > 0 {
                highlight_types.append(&mut vec![HighlightType::Comment; comment_len]);
                i += comment_len;
                continue;
            }

            highlight_types.push(HighlightType::Standard);
            i += 1;
        }

        // Apply colours
        for line in &mut *lines {
            line.highlight_types = Vec::with_capacity(line.content.len());
        }        
        let mut line_index = 0;
        for i in 0..chars.len() {
            if chars[i] == '\n' {
                line_index += 1;
            }else{
                lines[line_index].highlight_types.push(highlight_types[i]);
            }
        }
    }

    fn syntax_colour(&self, highlight_type: &HighlightType) -> Color {
        match highlight_type {
            HighlightType::Identity => Color::Cyan,
            HighlightType::Keyword => Color::Blue,
            HighlightType::Number => Color::Yellow,
            HighlightType::Bracket => Color::DarkYellow,
            HighlightType::String => Color::Red,
            HighlightType::Comment => Color::DarkGreen,
            HighlightType::SearchResult => Color::Magenta,
            _ => Color::Reset
        }
    }
}

pub struct Line {
    content: String,
    highlight_types: Vec<HighlightType>,
}

impl Line {
    pub fn new(content: String) -> Self {
        Self{content: content, highlight_types: Vec::new()}
    }

    pub fn blank() -> Self {
        Self{content: String::new(), highlight_types: Vec::new()}
    }

    pub fn insert(&mut self, index: usize, s: &str) {
        self.content.insert_str(index, s);
    }

    pub fn delete_char(&mut self, index: usize) {
        self.content.remove(index);
    }

    pub fn append(&mut self, line: &Line) {
        self.content.push_str(line.content.as_str());
    }

    pub fn split_at(&mut self, index: usize) -> Line {
        let new_line: String = self.content[index..].into();
        self.content = self.content[..index].into();
        Line::new(new_line)
    }

    pub fn find_phrase(&self, phrase: &str, start: usize) -> Option<usize> {
        self.content[start..].find(phrase)
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    fn print(&self, w: &mut Stdout, start: usize, end: usize, highlight: &Option<Box<dyn SyntaxHighlight>>) -> std::io::Result<()> {
        let start = min(start as usize, self.len());
        let end = min(end as usize , self.len());
        let mut previous_colour = Color::Reset;
        for (i, c) in self.content[start..end].chars().enumerate() {
            let colour = match (highlight, self.highlight_types.get(i + start)) {
                (Some(syntax_highlight),Some(highlight_type)) => {
                    syntax_highlight.syntax_colour(highlight_type)
                }
                _ => {
                    Color::Reset
                }   
            };
            if previous_colour != colour {
                queue!(w, SetForegroundColor(colour))?;
            }
            previous_colour = colour;
            queue!(w, style::Print(c))?;
        }
        queue!(w, SetForegroundColor(Color::Reset))?;
        Ok(())
    }
}

pub struct Text {
    lines: Vec<Line>,
    syntax_highlight: Option<Box<dyn SyntaxHighlight>>,
}

impl Text{
    pub fn new() -> Self {
        Self{lines: vec![Line::blank()], syntax_highlight: Some(Box::new(RustSyntax{})) }
    }

    pub fn load(&mut self, content: std::io::Result<String>) {
        self.lines = match content {
            Ok(contents) => {
                let mut lines: Vec<Line> = contents.lines().map(|it| Line::new(it.into())).collect();
                if lines.len() == 0 {lines.push(Line::blank())}
                lines
            },
            _ => vec![Line::blank()]
        };
        self.update_syntax();
    }

    pub fn reset(&mut self) {
        self.lines = vec![Line::blank()];
    }

    fn update_syntax(&mut self) {
        if let Some(syntax_highlight) = &self.syntax_highlight {
            syntax_highlight.update_syntax(&mut self.lines);
        }
    }

    pub fn save(&mut self, file_name: &String) -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new().write(true).create(true).open(file_name)?;
        let strings: Vec<String> = self.lines.iter().map(|it| it.content.clone()).collect();
        let contents = strings.join("\n");
        file.set_len(contents.len() as u64)?;
        file.write_all(contents.as_bytes())
    }

    pub fn print_line(&self, w: &mut Stdout, index: usize, start: u16, end: u16) -> std::io::Result<()> {
        if index < self.lines.len() {
            let line = &self.lines[index];
            line.print(w, start as usize, end as usize, &self.syntax_highlight)?;
        }
        Ok(())
    }

    pub fn insert_char(&mut self, c: char, cursor: &mut Cursor) {
        let (x, y) = cursor.get_position();
        let line = &mut self.lines[cursor.get_line_index()];
        match c{
            '\t' => {
                line.insert(x as usize, "    ");
                cursor.set_position(x + 4, y)
            }
            _ => {
                line.insert(x as usize, c.to_string().as_str());
                cursor.set_position(x + 1, y)
            }
        }
        self.update_syntax();
    }

    pub fn new_line(&mut self, cursor: &mut Cursor) {
        let line_index = cursor.get_line_index();
        let (x,y) = cursor.get_position();
        let new_line = self.lines[line_index].split_at(x as usize);
        self.lines.insert(line_index + 1, new_line);
        self.update_syntax();
        cursor.set_position(0, y + 1);
    }

    pub fn delete_char(&mut self, cursor: &mut Cursor) {
        let line_index = cursor.get_line_index();
        let (x, y) = cursor.get_position();
        if x > 0 {
            self.lines[line_index].delete_char(x as usize - 1);
            cursor.set_position(x - 1, y);
        }else if y > 0 {
            let old_line = self.lines.remove(line_index);
            let old_length = self.lines[line_index-1].len();
            self.lines[line_index-1].append(&old_line);
            cursor.set_position(old_length as u16, y-1);
        }
        self.update_syntax();
    }

    fn find_phrase(&self, phrase: &str, index: usize, start: usize) -> Option<usize> {
        self.lines[index].find_phrase(phrase, start)
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    fn line_len(&self, index: usize) -> usize {
        if index < self.len() {
            self.lines[index].len()
        } else {
            0
        }
    }
}