pub(super) fn has_mutating_redirection(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'>' {
            index += 1;
            continue;
        }

        let mut fd_start = index;
        while fd_start > 0 && bytes[fd_start - 1].is_ascii_digit() {
            fd_start -= 1;
        }
        let fd = &command[fd_start..index];
        let (target_start, target_end) = redirection_target_range(command, index);
        let target = &command[target_start..target_end];

        if !is_non_mutating_redirection_target(fd, target) {
            return true;
        }
        index = target_end.max(index + 1);
    }
    false
}

fn redirection_target_range(command: &str, index: usize) -> (usize, usize) {
    let bytes = command.as_bytes();
    let mut target_start = index + 1;
    if bytes.get(target_start) == Some(&b'>') {
        target_start += 1;
    }
    while bytes
        .get(target_start)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        target_start += 1;
    }
    let target_end = if bytes.get(target_start) == Some(&b'&') {
        let mut end = target_start + 1;
        while bytes.get(end).is_some_and(|byte| byte.is_ascii_digit()) {
            end += 1;
        }
        end
    } else {
        command[target_start..]
            .find(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ';' | '|' | '&'))
            .map(|offset| target_start + offset)
            .unwrap_or(command.len())
    };
    (target_start, target_end)
}

fn is_non_mutating_redirection_target(fd: &str, target: &str) -> bool {
    let stream_redirect = target
        .strip_prefix('&')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()));
    let discard = target == "/dev/null";

    stream_redirect || (discard && matches!(fd, "" | "1" | "2"))
}
