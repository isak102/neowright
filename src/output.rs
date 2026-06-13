use std::io::{self, Write};

pub fn write_success(writer: &mut impl Write, message: &str) -> io::Result<()> {
    writeln!(writer, "### Status")?;
    writeln!(writer, "{}", message)?;
    Ok(())
}

pub fn write_error(writer: &mut impl Write, message: impl AsRef<str>) -> io::Result<()> {
    writeln!(writer, "### Error")?;
    writeln!(writer, "{}", message.as_ref().trim())?;
    Ok(())
}
