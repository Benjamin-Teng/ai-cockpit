//! 給 named pipe／unix socket 共用的「讀寫各半」NDJSON stream 實作，以及子程序橋接
//! 共用的一行讀取邏輯。

use async_trait::async_trait;
use tokio::io::{
    AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadHalf, WriteHalf,
};

use super::NdjsonStream;

/// 用 `tokio::io::split` 把一個同時可讀可寫的連線（named pipe client、unix stream）拆成
/// 讀寫兩半，實作 `NdjsonStream`。
pub(crate) struct SplitLineStream<T> {
    reader: BufReader<ReadHalf<T>>,
    writer: WriteHalf<T>,
}

impl<T> SplitLineStream<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub(crate) fn new(io: T) -> Self {
        let (reader, writer) = tokio::io::split(io);
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }
}

#[async_trait]
impl<T> NdjsonStream for SplitLineStream<T>
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        write_line(&mut self.writer, line).await
    }

    async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
        read_one_line(&mut self.reader).await
    }
}

/// 送一行：補 `\n` 並 flush（`NdjsonStream::send_line` 的共用實作）。
pub(crate) async fn write_line<W>(writer: &mut W, line: &str) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

/// 讀一行、去掉尾端換行（含 `\r\n`）；乾淨 EOF（`read_line` 回 0 bytes）回 `Ok(None)`。
pub(crate) async fn read_one_line<R>(reader: &mut BufReader<R>) -> std::io::Result<Option<String>>
where
    R: AsyncRead + Unpin,
{
    let mut buf = String::new();
    let n = reader.read_line(&mut buf).await?;
    if n == 0 {
        return Ok(None);
    }
    if buf.ends_with('\n') {
        buf.pop();
        if buf.ends_with('\r') {
            buf.pop();
        }
    }
    Ok(Some(buf))
}
