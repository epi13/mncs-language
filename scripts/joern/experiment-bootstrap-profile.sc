import io.shiftleft.semanticcpg.language.*

@main def experimentBootstrapProfile(cpgFile: String): Unit = {
  importCpg(cpgFile)

  println("MNCS_LANGUAGE_EXPERIMENT_BOOTSTRAP_PROFILE")
  val targets = List(
    "lex",
    "parse",
    "function",
    "statement",
    "expression",
    "primary",
    "elaborate_program",
    "elaborate_function",
    "elaborate_statement",
    "elaborate_expr",
    "validate_operation",
    "lower_executable_body",
    "lower_body",
    "execute_instruction",
    "execute_backend",
    "execute_portable_wasm",
    "execute_research_bytecode",
    "run_experiment"
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
      path.endsWith("source.rs") || path.endsWith("frontend.rs") ||
      path.endsWith("body.rs") || path.endsWith("ir.rs") ||
      path.endsWith("ssa.rs") || path.endsWith("ssa_execution.rs") ||
      path.endsWith("mncs-codegen/src/lib.rs") || path.endsWith("mncs-cli/src/main.rs")
    ))
    .filter(call => call.name.matches(
      "resolve|bind|elaborate_.*|validate.*|lower_.*|execute.*|run_experiment|backend_adapter"
    ))
    .map(call => call.file.name.headOption.getOrElse("?") + ":" + call.method.name + ":" + call.name + ":" + call.lineNumber.getOrElse(-1))
    .l.sorted.foreach(value => println(s"PROFILE_BOUNDARY|$value"))
}
