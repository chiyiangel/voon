/* Mock library. Folder names come from the owner's scan of /Volumes/NAS1/Music (2026-11). Titles, artists and
   track lists are what Voon would derive from tags, CUE sheets and folder names; durations are illustrative. */

export type Source = "tag" | "cue" | "dir" | "manual";
export type Album = {
  id: string;
  title: string;
  artist: string;
  composer?: string;
  year?: string;
  format: string;
  spec: string;
  kind: "PCM" | "DSD";
  discs: number;
  tracks: number;
  size: string;
  dir: string;
  hue: number;
  hue2?: number;
  ini: string;
  lang: string;
  genre?: string;
  noCover?: boolean;
  catalog?: string;
};

export const ROOT = "/Volumes/NAS1/Music";

export const STATS = {
  files: "5,658",
  audioDirs: 707,
  albums: 586,
  collections: 9,
  size: "450.6 GB",
  inbox: 23,
  lastScan: "11月10日 22:14",
};

export const A: Record<string, Album> = {
  suwanai: { id: "suwanai", title: "J.S. Bach: Sonatas and Partitas for Solo Violin", artist: "諏訪内晶子 Akiko Suwanai", composer: "J.S. Bach", year: "2022", format: "FLAC", spec: "24/192", kind: "PCM", discs: 2, tracks: 31, size: "3.9 GB", dir: "Akiko Suwanai - J.S. Bach_Sonatas and Partitas for Solo Violin (2022) [24-192]", hue: 32, hue2: 8, ini: "BWV", lang: "器乐", genre: "古典" },
  kv563: { id: "kv563", title: "MOZART Divertimento KV 563", artist: "2L", composer: "W.A. Mozart", year: "2017", format: "FLAC", spec: "DXD 352.8k", kind: "PCM", discs: 1, tracks: 6, size: "4.6 GB", dir: "2L-148 MOZART Divertimento KV 563 [352.8kHz DXD]", hue: 200, hue2: 230, ini: "KV", lang: "器乐", genre: "古典", catalog: "2L-148" },
  evanescence: { id: "evanescence", title: "Evanescence", artist: "Evanescence", year: "2011", format: "FLAC", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 12, size: "421 MB", dir: "2011 Evanescence", hue: 260, hue2: 290, ini: "EV", lang: "英语", genre: "摇滚" },
  aga: { id: "aga", title: "Luna (2nd edition)", artist: "AGA 江海迦", year: "2016", format: "FLAC", spec: "24/96", kind: "PCM", discs: 1, tracks: 11, size: "1.1 GB", dir: "AGA - Luna (2nd edition)", hue: 220, hue2: 270, ini: "AGA", lang: "粤语", genre: "流行" },
  bachdsd: { id: "bachdsd", title: "巴赫：双小提琴协奏曲", artist: "群星", composer: "J.S. Bach", format: "DSF", spec: "DSD256", kind: "DSD", discs: 1, tracks: 12, size: "9.8 GB", dir: "巴赫：双小提琴协奏曲 (11.2MHz DSD)", hue: 12, hue2: 340, ini: "巴赫", lang: "器乐", genre: "古典" },
  brandenburg: { id: "brandenburg", title: "Bach: Brandenburg Concertos · Violin Concertos", artist: "I Musici", composer: "J.S. Bach", year: "1993", format: "APE", spec: "16/44.1", kind: "PCM", discs: 2, tracks: 27, size: "1.2 GB", dir: "Philips Duo/[438317-2]Bach Brandenburg Concertos [I Musici]", hue: 150, hue2: 120, ini: "BWV 1046", lang: "器乐", genre: "古典", catalog: "438 317-2" },
  emiri: { id: "emiri", title: "classique", artist: "宮本笑里 Emiri Miyamoto", year: "2007", format: "FLAC", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 18, size: "512 MB", dir: "宮本笑里 - classique", hue: 330, hue2: 300, ini: "笑里", lang: "器乐", genre: "跨界" },
  tongli: { id: "tongli", title: "粤语十大金曲 HQCDⅡ", artist: "童丽", year: "2010", format: "WAV", spec: "24/44.1", kind: "PCM", discs: 1, tracks: 12, size: "702 MB", dir: "童丽 - 粤语十大金曲 HQCDⅡ 24Bit", hue: 0, hue2: 30, ini: "童丽", lang: "粤语", genre: "流行" },
  kleiber: { id: "kleiber", title: "Beethoven: Symphony No. 5 & 7", artist: "Carlos Kleiber, Wiener Philharmoniker", composer: "L. van Beethoven", year: "1996", format: "FLAC", spec: "24/88.2", kind: "PCM", discs: 1, tracks: 8, size: "1.4 GB", dir: "Ludwig van Beethoven - Symphony 5 & 7 (Carlos Kleiber, Weiner Philharmoniker) (1996) [24-88]", hue: 45, hue2: 15, ini: "V·VII", lang: "器乐", genre: "古典" },
  lisiecki: { id: "lisiecki", title: "Chopin: Complete Nocturnes", artist: "Jan Lisiecki", composer: "F. Chopin", year: "2021", format: "FLAC", spec: "24/96", kind: "PCM", discs: 2, tracks: 21, size: "2.1 GB", dir: "Jan Lisiecki - Chopin- Complete Nocturnes (2021) [24-96]", hue: 210, hue2: 190, ini: "Op.9", lang: "器乐", genre: "古典" },
  wukong: { id: "wukong", title: "《黑神话：悟空》交响音乐会纪念专辑", artist: "游戏科学 & 8082Audio", year: "2024", format: "FLAC", spec: "24/48", kind: "PCM", discs: 1, tracks: 18, size: "1.3 GB", dir: "游戏科学 & 8082Audio - 《黑神话：悟空》交响音乐会纪念专辑 (2024) [FLAC]", hue: 28, hue2: 0, ini: "悟空", lang: "器乐", genre: "原声" },
  quiet: { id: "quiet", title: "Quiet Winter Night", artist: "Hoff Ensemble", year: "2012", format: "DSF", spec: "DSD256", kind: "DSD", discs: 1, tracks: 15, size: "12.4 GB", dir: "Quiet Winter Night (11.2MHz DSD)", hue: 205, hue2: 240, ini: "QWN", lang: "器乐", genre: "爵士" },
  karajan9: { id: "karajan9", title: "Beethoven: 9 Symphonies (1963)", artist: "Herbert von Karajan, Berliner Philharmoniker", composer: "L. van Beethoven", year: "2014", format: "FLAC", spec: "24/96", kind: "PCM", discs: 5, tracks: 37, size: "6.2 GB", dir: "Herbert von Karajan - Beethoven  9 Symphonies (1963, 2014) FLAC 24 96 (HDtracks)", hue: 10, hue2: 40, ini: "IX", lang: "器乐", genre: "古典" },
  langlang: { id: "langlang", title: "The Mozart Album", artist: "郎朗 Lang Lang", composer: "W.A. Mozart", year: "2014", format: "FLAC", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 19, size: "640 MB", dir: "郎朗：The Mozart Album", hue: 350, hue2: 20, ini: "郎朗", lang: "器乐", genre: "古典" },
  guzheng: { id: "guzheng", title: "鼓宴 · 须弥三国", artist: "须弥乐团", year: "2021", format: "DFF", spec: "DSD64", kind: "DSD", discs: 1, tracks: 11, size: "2.6 GB", dir: "鼔宴 - 须弥三国 (Chinese Drum - A Revival Of The Three Kingdoms)  2021 [DSD64]", hue: 0, hue2: 20, ini: "鼓宴", lang: "器乐", genre: "民乐" },
  twelve: { id: "twelve", title: "Beautiful Energy", artist: "女子十二乐坊", year: "2003", format: "WAV", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 14, size: "612 MB", dir: "女子十二乐坊《Beautiful Energy》原版[WAV+CUE]", hue: 300, hue2: 330, ini: "12", lang: "器乐", genre: "民乐" },
  kuraki1: { id: "kuraki1", title: "MAI KURAKI BEST 151A -LOVE & HOPE- Disc 1", artist: "倉木麻衣", year: "2014", format: "FLAC", spec: "24/96", kind: "PCM", discs: 1, tracks: 15, size: "1.6 GB", dir: "[Hires]倉木麻衣 - MAI KURAKI BEST 151A -LOVE & HOPE- Disc 1[24bit 96khz FLAC]", hue: 190, hue2: 160, ini: "151A", lang: "日语", genre: "流行" },
  kuraki2: { id: "kuraki2", title: "MAI KURAKI BEST 151A -LOVE & HOPE- Disc 2", artist: "倉木麻衣", year: "2014", format: "FLAC", spec: "24/96", kind: "PCM", discs: 1, tracks: 15, size: "1.5 GB", dir: "[Hires]倉木麻衣 - MAI KURAKI BEST 151A -LOVE & HOPE- Disc 2[24bit 96khz FLAC]", hue: 190, hue2: 160, ini: "151A", lang: "日语", genre: "流行" },
  vikingur: { id: "vikingur", title: "Mozart & Contemporaries", artist: "Víkingur Ólafsson", composer: "W.A. Mozart", year: "2021", format: "FLAC", spec: "24/192", kind: "PCM", discs: 1, tracks: 21, size: "3.4 GB", dir: "Víkingur Olafsson - Mozart & Contemporaries (2021) [24-192]", hue: 180, hue2: 210, ini: "VÓ", lang: "器乐", genre: "古典" },
  mozartvn: { id: "mozartvn", title: "MOZART Violin Concertos", artist: "Marianne Thorsen, TrondheimSolistene", composer: "W.A. Mozart", year: "2018", format: "DSF", spec: "DSD256", kind: "DSD", discs: 1, tracks: 9, size: "8.7 GB", dir: "【SONY·DSD】2L - MOZART Violin Concertos (11.2MHz DSD) 2018.03.09", hue: 230, hue2: 200, ini: "KV 216", lang: "器乐", genre: "古典" },
  brahms: { id: "brahms", title: "Brahms: Complete Piano Concertos & Overtures", artist: "Claudio Arrau", composer: "J. Brahms", year: "1993", format: "FLAC", spec: "16/44.1", kind: "PCM", discs: 2, tracks: 8, size: "1.1 GB", dir: "Philips Duo/[438320-2]Brahms Complete Piano Concertos & Overtures [Arrau]", hue: 100, hue2: 140, ini: "Op.15", lang: "器乐", genre: "古典", catalog: "438 320-2" },
  grumiaux: { id: "grumiaux", title: "Mozart: Complete Violin Concertos", artist: "Arthur Grumiaux", composer: "W.A. Mozart", year: "1993", format: "APE", spec: "16/44.1", kind: "PCM", discs: 2, tracks: 18, size: "1.0 GB", dir: "Philips Duo/[438323-2]Mozart Complete Violin Concertos [Graumiaux]", hue: 130, hue2: 90, ini: "K.219", lang: "器乐", genre: "古典", catalog: "438 323-2" },
  hires: { id: "hires", title: "Hi-res Selection", artist: "群星", format: "FLAC", spec: "24/96", kind: "PCM", discs: 1, tracks: 500, size: "18.3 GB", dir: "Hi-res Selection", hue: 0, ini: "?", lang: "多语种", noCover: true },
  numeric: { id: "numeric", title: "Unknown_Artist / 0015830558", artist: "未知", format: "FLAC", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 14, size: "498 MB", dir: "Unknown_Artist/[未命名]", hue: 0, ini: "?", lang: "未知", noCover: true },
  sens: { id: "sens", title: "SENS", artist: "S.E.N.S.", format: "APE", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 0, size: "512 MB", dir: "SENS", hue: 175, hue2: 150, ini: "SENS", lang: "器乐", genre: "新世纪" },
  group2007: { id: "group2007", title: "2007十大流行发烧唱片", artist: "群星", year: "2007", format: "APE", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 0, size: "498 MB", dir: "群星.-.[2007十大流行发烧唱片].专辑.(APE)", hue: 50, ini: "2007", lang: "国语", noCover: true },
  pink: { id: "pink", title: "Funhouse", artist: "P!nk", year: "2008", format: "FLAC", spec: "16/44.1", kind: "PCM", discs: 1, tracks: 13, size: "420 MB", dir: "Pink - Funhouse (2008) [FLAC]", hue: 320, hue2: 350, ini: "P!", lang: "英语", genre: "流行" },
  kempff: { id: "kempff", title: "Beethoven: Sonatas Nos. 1-6 (Remastered)", artist: "Wilhelm Kempff", composer: "L. van Beethoven", year: "2019", format: "FLAC", spec: "24/96", kind: "PCM", discs: 1, tracks: 22, size: "1.9 GB", dir: "Wilhelm Kempff - Beethoven- Sonatas Nos. 1 - 6 (Remastered) (2019) [24-96]", hue: 60, hue2: 30, ini: "Op.2", lang: "器乐", genre: "古典" },
  zimerman: { id: "zimerman", title: "Beethoven: Complete Piano Concertos", artist: "Krystian Zimerman, Simon Rattle, LSO", composer: "L. van Beethoven", year: "2021", format: "FLAC", spec: "24/96", kind: "PCM", discs: 3, tracks: 15, size: "3.2 GB", dir: "Krystian Zimerman - Beethoven- Complete Piano Concertos Hi-Res", hue: 25, hue2: 55, ini: "Op.73", lang: "器乐", genre: "古典" },
  minnaar: { id: "minnaar", title: "Beethoven: Piano Concertos Nos. 1 and 2", artist: "Hannes Minnaar, Jan Willem de Vriend", composer: "L. van Beethoven", year: "2016", format: "DSF", spec: "DSD256", kind: "DSD", discs: 1, tracks: 6, size: "7.9 GB", dir: "Beethoven - Piano Concertos Nos. 1 and 2 - Hannes Minnaar, Jan Willem de Vriend (2016) DSD256", hue: 270, hue2: 240, ini: "Op.15", lang: "器乐", genre: "古典" },
  wangleehom: { id: "wangleehom", title: "力宏二十 二十周年唯一精选", artist: "王力宏", year: "2015", format: "FLAC", spec: "24/96", kind: "PCM", discs: 2, tracks: 26, size: "2.9 GB", dir: "[Hires]王力宏《力宏二十 二十周年唯一精选》24-96", hue: 215, hue2: 185, ini: "20", lang: "国语", genre: "流行" },
  jade: { id: "jade", title: "皎月云间之梦 Jade Moon Upon a Sea of Clouds", artist: "HOYO-MiX", year: "2020", format: "FLAC", spec: "24/48", kind: "PCM", discs: 3, tracks: 64, size: "3.8 GB", dir: "原神-皎月云间之梦 Jade Moon Upon a Sea of Clouds (《原神》璃月篇OST)", hue: 42, hue2: 165, ini: "璃月", lang: "器乐", genre: "原声" },
  piano: { id: "piano", title: "Limitless", artist: "The Piano Guys", year: "2018", format: "FLAC", spec: "24/44.1", kind: "PCM", discs: 1, tracks: 13, size: "720 MB", dir: "Limitless - The Piano Guys [44.1kHz／24bit][FLAC]", hue: 195, hue2: 30, ini: "TPG", lang: "器乐", genre: "跨界" },
};

export const GRID_MAIN = ["suwanai", "kv563", "evanescence", "aga", "bachdsd", "brandenburg", "emiri", "tongli", "kleiber", "lisiecki", "wukong", "quiet", "karajan9", "langlang", "guzheng", "twelve", "vikingur", "mozartvn"];

export type Track = { n: string; name: string; d: string; src?: Source };
export const BRANDENBURG_DISCS: { label: string; files: string; tracks: Track[]; more?: string }[] = [
  {
    label: "CD1",
    files: "CD1/CDImage.ape + CDImage.cue",
    tracks: [
      { n: "1", name: "Brandenburg Concerto No. 1 in F, BWV 1046 — I. Allegro", d: "4:02" },
      { n: "2", name: "— II. Adagio", d: "3:58" },
      { n: "3", name: "— III. Allegro", d: "4:31" },
      { n: "4", name: "— IV. Menuetto – Trio – Polacca", d: "7:44" },
      { n: "5", name: "Brandenburg Concerto No. 2 in F, BWV 1047 — I. (Allegro)", d: "5:03" },
    ],
    more: "… 另 9 轨（CUE 虚拟音轨，原文件不拆分）",
  },
  {
    label: "CD2",
    files: "CD2/CDImage.ape + CDImage.cue",
    tracks: [
      { n: "1", name: "Brandenburg Concerto No. 4 in G, BWV 1049 — I. Allegro", d: "7:12" },
      { n: "2", name: "— II. Andante", d: "3:41" },
      { n: "3", name: "— III. Presto", d: "4:58" },
    ],
    more: "… 另 10 轨",
  },
];

export const KLEIBER_TRACKS: Track[] = [
  { n: "1", name: "Symphony No. 5 in C minor, Op. 67 — I. Allegro con brio", d: "7:22", src: "tag" },
  { n: "2", name: "— II. Andante con moto", d: "9:55", src: "tag" },
  { n: "3", name: "— III. Allegro", d: "5:12", src: "tag" },
  { n: "4", name: "— IV. Allegro", d: "11:04", src: "tag" },
  { n: "5", name: "Symphony No. 7 in A, Op. 92 — I. Poco sostenuto – Vivace", d: "13:43", src: "tag" },
  { n: "6", name: "— II. Allegretto", d: "8:26", src: "tag" },
  { n: "7", name: "— III. Presto", d: "8:21", src: "tag" },
  { n: "8", name: "— IV. Allegro con brio", d: "8:41", src: "tag" },
];

export const SUWANAI_TRACKS: Track[] = [
  { n: "1", name: "Sonata No. 1 in G minor, BWV 1001 — I. Adagio", d: "4:11", src: "tag" },
  { n: "2", name: "— II. Fuga. Allegro", d: "5:22", src: "tag" },
  { n: "3", name: "— III. Siciliana", d: "3:02", src: "tag" },
  { n: "4", name: "— IV. Presto", d: "3:31", src: "tag" },
  { n: "5", name: "Partita No. 1 in B minor, BWV 1002 — I. Allemanda", d: "5:48", src: "tag" },
];

export const COMPOSERS = [
  { id: "mozart", name: "W.A. Mozart", cn: "莫扎特", n: 214, hue: 210 },
  { id: "beethoven", name: "L. van Beethoven", cn: "贝多芬", n: 61, hue: 20 },
  { id: "bach", name: "J.S. Bach", cn: "巴赫", n: 23, hue: 150 },
  { id: "chopin", name: "F. Chopin", cn: "肖邦", n: 9, hue: 260 },
  { id: "brahms", name: "J. Brahms", cn: "勃拉姆斯", n: 8, hue: 100 },
  { id: "tchaikovsky", name: "P.I. Tchaikovsky", cn: "柴可夫斯基", n: 7, hue: 0 },
  { id: "schubert", name: "F. Schubert", cn: "舒伯特", n: 5, hue: 300 },
  { id: "kreisler", name: "F. Kreisler", cn: "克莱斯勒", n: 3, hue: 45 },
  { id: "mahler", name: "G. Mahler", cn: "马勒", n: 2, hue: 190 },
  { id: "hisaishi", name: "久石让", cn: "Joe Hisaishi", n: 2, hue: 330 },
];
