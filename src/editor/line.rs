
#[derive(Debug)]
pub struct Line{
    pub start: usize,
    pub end: usize,
    pub size: usize,
}

impl Line{

    pub fn new(start_index : usize, end_index : usize) -> Self{
        Self{
            start: start_index,
            end : end_index,
            size: (end_index - start_index)+1,
        }
    }
    pub fn compute_lines(buffer: &str) -> Vec<Line>{
        let mut result = Vec::new();
        let values = buffer.chars();
        let mut start = 0;
        let mut end = 0;
        for c in values{
            if c == '\n'{
                result.push(Line::new(start,end));
                start = end+1;
                end = start;
                continue;
            }
            end += 1;
        }
        result.push(Line::new(start,end));
        result
    }


}
