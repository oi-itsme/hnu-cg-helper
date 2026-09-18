// 程序填空题提交脚本：buildPlan(input) → SubmissionPlan
//
// input = {
//   descriptor: parse() 产出的 submission 描述（含 hidden_fields、gaps）,
//   answers: { answer1: "...", answer2: "...", ... },
//   wtime: 耗时秒数
// }
//
// CG 填空题提交是 urlencoded 表单：hidden 字段回带 + 各空位答案。

function buildPlan(input) {
  var d = input.descriptor;
  var fields = {};

  // hidden 字段原样回带（doSubmit/byCE/progLanguage/problemID/assignID 等）
  var hidden = d.hidden_fields || {};
  for (var k in hidden) fields[k] = hidden[k];

  // wtime 覆盖为真实耗时
  fields.wtime = String(Math.floor(input.wtime || 0));

  // 空位答案
  var answers = input.answers || {};
  var gaps = d.gaps || [];
  for (var i = 0; i < gaps.length; i++) {
    var name = gaps[i].name;
    fields[name] = answers[name] || '';
  }

  return {
    method: 'POST',
    url: 'assignment/showProcessMsg.jsp',
    body: {
      type: 'form',
      fields: fields
    }
  };
}
