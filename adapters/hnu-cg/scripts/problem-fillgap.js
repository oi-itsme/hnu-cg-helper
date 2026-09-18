// 程序填空题（programFillGapList.jsp）解析脚本
// 输入：区域提取 + 脱敏后的合成文档（仅含题面/提交表单）
// 输出：ProblemPageOutput { statement_html, statement_text, submission }
//
// 宿主 API 同 problem-program.js，见该文件头注释。

function parse() {
  var contentEl = select('.cgProblemContentClass');
  if (contentEl < 0) throw new Error('未找到题面区域 .cgProblemContentClass');

  var rawHtml = html(contentEl);
  var statementHtml = cleanHtml(rawHtml.replace(/&nbsp;/g, ' '));
  var statementText = text(contentEl).replace(/\s+/g, ' ').trim();

  var form = select('form#uploadFORM');
  if (form < 0) throw new Error('未找到提交表单 form#uploadFORM');

  // hidden 字段原样收集（提交时回带）
  var hidden = {};
  var inputs = selectAll('form#uploadFORM input[type=hidden]');
  for (var i = 0; i < inputs.length; i++) {
    var name = attr(inputs[i], 'name');
    if (name) hidden[name] = attr(inputs[i], 'value') || '';
  }
  if (!hidden.problemID || !hidden.assignID) {
    throw new Error('表单缺少 problemID/assignID hidden 字段');
  }

  // 代码骨架 + 空位：一次文档序遍历，code.cgcode 为代码片段，textarea 为空位
  var gaps = [];
  var skeleton = [];
  var parts = selectAll('form#uploadFORM code.cgcode, form#uploadFORM textarea');
  for (var j = 0; j < parts.length; j++) {
    var tag = attr(parts[j], 'name');
    if (tag) {
      // textarea：空位
      gaps.push({ name: tag });
      skeleton.push({ type: 'gap', name: tag });
    } else {
      // code：代码片段（text() 已解码实体， NBSP 归一化为空格）
      var code = text(parts[j]).replace(/\u00a0/g, " ");
      if (code.trim().length > 0) skeleton.push({ type: 'code', text: code });
    }
  }
  if (gaps.length === 0) throw new Error('未找到任何填空 textarea');

  var lang = hidden.progLanguage || '';

  return {
    statement_html: statementHtml,
    statement_text: statementText,
    submission: {
      kind: 'fill_gap',
      languages: [{ value: lang, label: lang }],
      gaps: gaps,
      skeleton: skeleton,
      hidden_fields: hidden,
      problem_id: parseInt(hidden.problemID, 10),
      assign_id: parseInt(hidden.assignID, 10)
    }
  };
}
