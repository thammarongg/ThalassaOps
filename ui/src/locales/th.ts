const th = {
  health: {
    eyebrow: "เชลล์ภายใน ThalassaOps",
    title: "สถานะระบบ",
    checking: "กำลังตรวจสอบแกนระบบภายใน…",
    error: "การตรวจสอบสถานะล้มเหลว",
    policyVersion: "รุ่นนโยบาย"
  },
  status: {
    healthy: "พร้อมใช้งาน",
    degraded: "ประสิทธิภาพลดลง",
    unavailable: "ไม่พร้อมใช้งาน",
    warning: "คำเตือน",
    critical: "วิกฤต"
  },
  severity: {
    s1: "S1 วิกฤต",
    s2: "S2 ร้ายแรง",
    s3: "S3 ปานกลาง",
    s4: "S4 เล็กน้อย",
    s5: "S5 ข้อมูล"
  },
  demo: {
    title: "ตัวอย่างระบบออกแบบ",
    primaryCard: "สถานะการปฏิบัติการ",
    secondaryCard: "สถานะว่างที่ใช้ซ้ำได้",
    emptyTitle: "ยังไม่ได้เลือกหลักฐาน",
    tableCaption: "ข้อมูลสภาพแวดล้อมตัวอย่าง",
    name: "ชื่อ",
    firstTab: "ภาพรวม",
    secondTab: "หลักฐาน",
    timelineEvent: "ได้รับสัญญาณ",
    commandLabel: "พื้นผิวคำสั่ง",
    commandPlaceholder: "ค้นหาคำสั่ง",
    drawerTitle: "ลิ้นชักคอมโพเนนต์",
    close: "ปิด",
    timelineTitle: "เส้นคลื่นหลักฐาน",
    healthCard: "สถานะแกนระบบ"
  }
} as const;

export default th;
