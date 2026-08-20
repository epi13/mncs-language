import io.shiftleft.semanticcpg.language.*

@main def sourceVerticalSlice(cpgFile: String): Unit = {
  importCpg(cpgFile)

  println("MNCS_LANGUAGE_SOURCE_VERTICAL_SLICE")
  val targets = List(
    "identity_is_valid",
    "lex",
    "parse",
    "document",
    "function",
    "front_end",
    "elaborate_program",
    "run_source_study",
    "front_end_pass_executions",
    "compile",
    "run_study"
  )
  targets.foreach { name =>
    val methods = cpg.method.nameExact(name).filter(_.filename.endsWith(".rs")).l
    val files = methods.map(_.filename).distinct.sorted.mkString(",")
    val callers = methods.flatMap(_.callIn.method.name.l).distinct.sorted.mkString(",")
    val callees = methods.flatMap(_.callOut.name.l).distinct.sorted.mkString(",")
    val controls = methods.flatMap(_.controlStructure.controlStructureType.l)
      .groupBy(identity).view.mapValues(_.size).toMap.toSeq.sortBy(_._1).mkString(",")
    println(s"METHOD|$name|count=${methods.size}|files=$files|callers=$callers|callees=$callees|controls=$controls")
  }

  cpg.call
    .filter(_.file.name.headOption.exists(path =>
      path.endsWith("source.rs") || path.endsWith("frontend.rs") || path.endsWith("mncs-compiler/src/lib.rs")
    ))
    .filter(call => call.name.matches(
      "identity_is_valid|lex|parse|elaborate_program|validate|semantic_graph|semantic_identities|lower_to_ir|lower_to_ssa|run_study|seal"
    ))
    .map(call => call.file.name.headOption.getOrElse("?") + ":" + call.method.name + ":" + call.name + ":" + call.lineNumber.getOrElse(-1))
    .l.sorted.foreach(value => println(s"STAGE_BOUNDARY|$value"))
}
