// 普通编程题提交脚本：buildPlan(input) → SubmissionPlan
//
// input = {
//   descriptor: parse() 产出的 submission 描述,
//   language: 用户选择的语言,
//   main_class: java 主类名（可选）,
//   code: 源代码内容（由宿主封装为 multipart 文件，本脚本无需读取）,
//   wtime: 耗时秒数
// }
//
// CG 的提交是 multipart 文件上传，参数全在 URL query 里
// （仿照页面内联脚本 filesubmit() 的拼接逻辑）。

function buildPlan(input) {
  var d = input.descriptor;

  var url = 'assignment/showProcessMsg.jsp'
    + '?problemID=' + d.problem_id
    + '&assignID=' + d.assign_id
    + '&doSubmit=true'
    + '&progLanguage=' + encodeURIComponent(input.language)
    + '&wtime=' + Math.floor(input.wtime || 0);

  // java 需要主类名（页面字段名即 javaMainCLass，CG 的拼写如此）
  if (input.language === 'java' && input.main_class) {
    url += '&javaMainCLass=' + encodeURIComponent(input.main_class);
  }

  return {
    method: 'POST',
    url: url,
    body: {
      type: 'multipart',
      file_field: 'FILE1',
      fields: {}
    }
  };
}
