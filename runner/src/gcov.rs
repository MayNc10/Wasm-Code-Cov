/// http://src.gnu-darwin.org/src/contrib/gcc/gcov-io.h.html
/// This is a module for ouput .gcda data files
/// These can be read by profilers that work with gcov (there are many)

const GCDA_MAGIC: Int32 = 0x67636461;

struct GcdaString {
    text: String,
}

impl GcdaString {
    pub fn new(s: String) -> GcdaString {
        todo!()
    }
}

type Int32 = u32;
type Int64 = (Int32, Int32);

enum Item {
    Int32(Int32),
    Int64(Int64),
    String(GcdaString),
}

struct Header {
    tag: Int32,
    length: Int32,
}

struct Unit {
    header: Header,
    checksum: Int32,
}

struct DataBlock {
    unit: Unit,
    function_data: Vec<FunctionData>,
    object: Summary,
    program: Summary,
}

struct FunctionData {
    announce_function: AnnounceFunction,
    arc_counts: ArcCounts,
}

struct AnnounceFunction {
    header: Header,
    ident: Int32,
    checksum: I32,
}

struct ArcCounts {
    header: Header,
    counts: Vec<Int64>,
}

struct Summary<const N: usize> {
    checksum: Int32,
    summaries: [CountSummary; N],
}

struct CountSummary {
    num: Int32,
    runs: Int32,
    sum: Int64,
    max: Int64,
    sum_max: Int64,
}

enum GcdaRecord {
    Data(Vec<DataBlock>),
    Unit(Unit),
    FunctionData(FunctionData),
    AnnounceFunction(AnnounceFunction),
    ArcCounts(ArcCounts),
    Summary(Summary),
    CountSummary(CountSummary),
}
