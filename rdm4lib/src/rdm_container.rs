use std::fmt;
use std::io::{Seek, SeekFrom, Write};
use std::ops::Deref;
use std::str;
use std::vec::Vec;

use std::marker::PhantomData;

use binrw::file_ptr::FilePtrArgs;
use binrw::{binread, binrw, BinRead, BinWrite, FilePtr32};

#[derive(Debug, BinRead)]
#[br(import_raw(c: u32))]
pub struct VectorN<T: for<'a> BinRead<Args<'a> = ()> + 'static> {
    #[br(count = c)]
    pub x: Vec<T>,
}

#[derive(Debug, BinRead)]
#[br(import_raw(c: u32))]
#[br(assert(c == 1,"Expected only 1 element of type {} but got {}",std::any::type_name::<T>(),c))]
pub struct Vector1<T: for<'a> BinRead<Args<'a> = ()> + 'static> {
    pub x: T,
}

// types that will be used to parameterize a type constructor
#[derive(Debug, BinRead)]
pub struct Fixed<T>(PhantomData<T>);
#[derive(Debug, BinRead)]
pub struct Dynamic<T>(PhantomData<T>);

// a type level function that says what kind of data corresponds to what type
pub trait VectorSize {
    type Data;
    fn len(data: &Self::Data) -> u32;
}

impl<T> VectorSize for Dynamic<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    type Data = VectorN<T>;
    fn len(data: &Self::Data) -> u32 {
        data.x.len() as u32
    }
}

impl<T> VectorSize for Fixed<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    type Data = Vector1<T>;
    fn len(_data: &Self::Data) -> u32 {
        1
    }
}

#[derive(Debug)]
#[binrw]
pub struct RdmContainerPrefix {
    pub count: u32,
    pub part_size: u32,
}

#[binread]
pub struct RdmContainer<const T_IS_PARTSIZED: bool, Z>
where
    Z: VectorSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
{
    //#[br(temp)]
    pub info: RdmContainerPrefix,

    //#[br(args_raw = c)]
    //#[br(assert(if T_IS_PARTSIZED {true} else {true}, "oops!"))]
    #[br(args_raw = match T_IS_PARTSIZED {
        true => info.count,
        false => info.count * info.part_size,
    })]
    pub e: Z::Data,
}

impl<Z> std::ops::Deref for RdmContainer<true, Fixed<Z>>
where
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    type Target = Z;
    fn deref(&self) -> &Self::Target {
        &self.e.x
    }
}

impl<Z> std::ops::DerefMut for RdmContainer<true, Fixed<Z>>
where
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.e.x
    }
}

impl<const T_IS_PARTSIZED: bool, Z> std::ops::Deref for RdmContainer<T_IS_PARTSIZED, Dynamic<Z>>
where
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    type Target = [Z];
    fn deref(&self) -> &Self::Target {
        &self.e.x
    }
}

impl<const T_IS_PARTSIZED: bool, Z> std::ops::DerefMut for RdmContainer<T_IS_PARTSIZED, Dynamic<Z>>
where
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.e.x
    }
}

impl RdmString {
    pub fn as_ascii(&self) -> &str {
        let (_head, body, _tail) = unsafe { self.deref().align_to::<u8>() };
        str::from_utf8(body).unwrap()
    }
}

impl fmt::Display for RdmString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ascii())
    }
}

impl fmt::Debug for RdmString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ascii())
    }
}

pub struct AnnoPtr<T>(pub FilePtr32<T>);

impl<T: BinRead> std::ops::Deref for AnnoPtr<T> {
    type Target = FilePtr32<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: BinRead> std::ops::DerefMut for AnnoPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: BinRead + std::fmt::Debug> std::fmt::Debug for AnnoPtr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.deref(), f)
    }
}

// https://docs.rs/binrw/0.11.2/binrw/docs/attribute/index.html#custom-parserswriters
// https://github.com/jam1garner/binrw/blob/8a49a5cea8568eed7b86a0e2646b8608527a60f4/binrw/src/file_ptr.rs#L127
impl<const N: bool, Z> BinRead for AnnoPtr<RdmContainer<N, Z>>
where
    Z: VectorSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
{
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let mut p: FilePtr32<RdmContainer<N, Z>> =
            <_>::read_options(reader, endian, FilePtrArgs::default()).unwrap();
        if p.ptr != 0 {
            p.ptr -= 8;

            p.after_parse(reader, endian, FilePtrArgs::default())
                .unwrap();

            return Ok(AnnoPtr(p));
        }
        // Err(binrw::Error::NoVariantMatch { pos: reader.stream_position().unwrap()-4 })
        // NullPtr
        Ok(AnnoPtr(FilePtr32 {
            ptr: 0,
            value: None,
        }))
    }
}

// https://docs.rs/binrw/0.11.2/binrw/docs/attribute/index.html#using-fileptrparse-to-read-a-nullstring-without-storing-a-fileptr
// https://docs.rs/binrw/0.11.2/binrw/file_ptr/struct.FilePtr.html#method.parse
impl<const N: bool, Z> AnnoPtr<RdmContainer<N, Z>>
where
    Z: VectorSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
    Z::Data: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    #[binrw::parser(reader, endian)]
    pub fn parse<Args>(_args: FilePtrArgs<Args>, ...) -> binrw::BinResult<RdmContainer<N, Z>>
    where
        Args: Clone,
        Z: for<'a> BinRead<Args<'a> = Args>,
    {
        let v = Self::read_options(reader, endian, ())
            .unwrap()
            .0
            .into_inner();
        Ok(v)
    }
}

impl<const N: bool, Z> BinWrite for AnnoPtr<RdmContainer<N, Z>>
where
    Z: VectorSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
    Z::Data: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    type Args<'a> = &'a mut u64;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        if self.0.value.is_none() {
            dbg!("Wrote null ptr!");
            0u32.write_options(writer, endian, ()).unwrap();
            return Ok(());
        }
        dbg!(*args);
        let _ptrptr: u64 = writer.stream_position().unwrap();
        (*args as u32 + 8)
            .write_options(writer, endian, ())
            .unwrap();

        let pos_end = writer.stream_position().unwrap();
        writer.seek(SeekFrom::Start(*args)).unwrap();
        let pointed_to_data = self.deref().deref();
        pointed_to_data
            .write_options(
                writer,
                endian,
                RdmContainerArgs {
                    ptr: None,
                    end_offset: 0,
                },
            )
            .unwrap();
        *args = writer.stream_position().unwrap();
        writer.seek(SeekFrom::Start(pos_end)).unwrap();

        Ok(())
    }
}
#[derive(Default)]
pub struct RdmContainerArgs {
    pub ptr: Option<u64>,
    pub end_offset: u64,
}

impl<const N: bool, Z> BinWrite for RdmContainer<N, Z>
where
    Z: VectorSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
    Z::Data: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    //type Args<'a> = Option<u64>;
    type Args<'a> = RdmContainerArgs;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        self.info.write_options(writer, endian, ())?;

        let pos_start = writer.stream_position().unwrap();
        // Todo: Args Off ?!?
        let mut end = args.end_offset + pos_start + (self.info.count * self.info.part_size) as u64;
        //dbg!(end);

        let p: &Z::Data = &self.e;
        assert!(Z::len(p) == self.info.count || !N);
        if Z::len(p) != self.info.count && N {
            dbg!(Z::len(p), self.info.count, self.info.part_size);
        }
        p.write_options(writer, endian, &mut end)?;

        let pos_end = writer.stream_position().unwrap();

        assert_eq!(
            pos_end - pos_start,
            (self.info.count * self.info.part_size) as u64
        );

        writer.seek(SeekFrom::Start(end)).unwrap();

        Ok(())
    }
}

impl<Z> BinWrite for VectorN<Z>
where
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
    Z: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    type Args<'a> = &'a mut u64;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        for x in self.x.iter() {
            x.write_options(writer, endian, args)?;
        }
        //self.x.write_options(writer, endian, ())?;
        Ok(())
    }
}

impl<Z> BinWrite for Vector1<Z>
where
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
    Z: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    type Args<'a> = &'a mut u64;

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        self.x.write_options(writer, endian, args)?;
        Ok(())
    }
}

#[binrw]
#[bw(import_raw(_dst: &mut u64))]
#[repr(transparent)]
pub struct AnnoChar(pub u8);

#[binrw]
#[bw(import_raw(_dst: &mut u64))]
#[repr(transparent)]
pub struct AnnoU8(pub u8);

#[binrw]
#[bw(import_raw(_dst: &mut u64))]
#[repr(transparent)]
pub struct AnnoU16(pub u16);

pub type RdmTypedT<T> = RdmContainer<true, Fixed<T>>;
pub type RdmTypedContainer<T> = RdmContainer<true, Dynamic<T>>;
pub type RdmUntypedContainer = RdmContainer<false, Dynamic<AnnoU8>>;
pub type RdmString = RdmContainer<true, Dynamic<AnnoChar>>;
