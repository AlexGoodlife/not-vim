
pub fn digits(mut num : usize) -> usize{
    if num == 0{
        return 1
    } 
    let mut result = 0;
    while num != 0 {
        num /= 10;
        result += 1;
    }
    result
}
